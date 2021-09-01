use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use bytemuck::{Pod, TransparentWrapper};

use crate::{
    lock::Lock,
    page::{Page, PageNr, NULL_PAGE_NR},
    tree::node::NodeRef,
};

use super::{
    branch::Branch,
    leaf::Leaf,
    node::{node, NodePage},
};

pub struct Cursor<'a, R: Deref<Target = PageNr>, K: Pod + Ord, V: Pod> {
    root: R,
    entries: Vec<Entry>,
    key: Option<&'a K>,
    _phantom: PhantomData<V>,
}

#[derive(Default, Clone, Copy)]
struct Entry {
    page_nr: PageNr,
    index: usize,
}

impl<'a, R: Deref<Target = PageNr>, K: Pod + Ord, V: Pod> Cursor<'a, R, K, V> {
    pub fn new(root: R, key: &'a K, lock: &Lock) -> Self {
        let mut entries = Vec::new();
        let mut page_nr = *root;
        let key = loop {
            if page_nr == NULL_PAGE_NR {
                break Some(key);
            }
            let page = unsafe { lock.page(page_nr) };
            match node::<K, V>(page) {
                NodeRef::Branch(branch) => {
                    let index = branch.search(key);
                    entries.push(Entry { page_nr, index });
                    page_nr = branch.children()[index];
                }
                NodeRef::Leaf(leaf) => match leaf.search(key) {
                    Ok(index) => {
                        entries.push(Entry { page_nr, index });
                        break None;
                    }
                    Err(index) => {
                        entries.push(Entry { page_nr, index });
                        break Some(key);
                    }
                },
            }
        };
        entries.reverse();
        Self {
            root,
            entries,
            key,
            _phantom: PhantomData,
        }
    }

    pub fn value<'b: 'a>(&self, lock: &'b Lock) -> Option<&'b V> {
        if self.key.is_none() {
            Some(&self.leaf(lock).values()[self.entries[0].index])
        } else {
            None
        }
    }

    pub fn has_value(&self) -> bool {
        self.key.is_none()
    }

    fn is_empty(&self) -> bool {
        self.height() == 0
    }

    fn height(&self) -> usize {
        self.entries.len()
    }

    fn root_level(&self) -> usize {
        self.height() - 1
    }

    fn is_valid(&self, lock: &Lock) -> bool {
        !self.is_empty() && self.entries[0].index < self.page(0, lock).len()
    }

    fn page<'b: 'a>(&self, level: usize, lock: &'b Lock) -> &'b Page {
        unsafe { lock.page(self.entries[level].page_nr) }
    }

    fn leaf<'b: 'a>(&self, lock: &'b Lock) -> &'b Leaf<K, V> {
        Leaf::wrap_ref(self.page(0, lock))
    }
}

impl<'a, R: DerefMut<Target = PageNr>, K: Pod + Ord, V: Pod> Cursor<'a, R, K, V> {
    #[allow(clippy::mut_from_ref)]
    unsafe fn leaf_mut<'b: 'a>(&mut self, lock: &'b Lock) -> &'b mut Leaf<K, V> {
        Leaf::wrap_mut(self.page_mut(0, lock))
    }

    #[allow(clippy::mut_from_ref)]
    unsafe fn branch_mut(&mut self, level: usize, lock: &'a Lock) -> &'a mut Branch<K> {
        Branch::wrap_mut(self.page_mut(level, lock))
    }

    #[allow(clippy::mut_from_ref)]
    unsafe fn page_mut<'b: 'a>(&mut self, level: usize, lock: &'b Lock) -> &'b mut Page {
        if let Some(page) = lock.try_page_mut(self.entries[level].page_nr) {
            page
        } else if level + 1 < self.entries.len() {
            let index = self.entries[level + 1].index;
            let parent = self.branch_mut(level + 1, lock);
            let child = &mut parent.children_mut()[index];
            let page = lock.page_mut(child);
            self.entries[level].page_nr = *child;
            page
        } else {
            let page = lock.page_mut(self.root.deref_mut());
            let root_level = self.root_level();
            self.entries[root_level].page_nr = *self.root;
            page
        }
    }

    pub unsafe fn value_mut<'b: 'a>(&mut self, lock: &'b Lock) -> Option<&'b mut V> {
        if self.key.is_none() {
            Some(&mut self.leaf_mut(lock).values_mut()[self.entries[0].index])
        } else {
            None
        }
    }

    pub unsafe fn set_value(&mut self, value: V, lock: &'a Lock) -> bool {
        if self.entries.is_empty() {
            if let Some(key) = self.key.take() {
                let (nr, leaf) = Leaf::allocate(lock);
                leaf.insert(0, *key, value);
                *self.root = nr;
                self.entries.push(Entry {
                    page_nr: nr,
                    index: 0,
                });
                return true;
            } else {
                return false;
            }
        }
        let leaf = self.leaf_mut(lock);
        let index = self.entries[0].index;
        if let Some(key) = self.key.take() {
            if leaf.len() < Leaf::<K, V>::order() {
                leaf.insert(index, *key, value);
            } else {
                let (page_nr, other) = Leaf::allocate(lock);
                leaf.split(other);
                let len = leaf.len();
                self.parent_insert(0, other.keys()[0], page_nr, lock);
                if index < len {
                    leaf.insert(index, *key, value);
                } else {
                    let index = index - len;
                    other.insert(index, *key, value);
                    self.entries[0] = Entry { page_nr, index };
                }
            }
        } else {
            if !self.is_valid(lock) {
                return false;
            }
            leaf.values_mut()[index] = value;
        }
        true
    }

    unsafe fn parent_insert(&mut self, level: usize, key: K, value: PageNr, lock: &'a Lock) {
        if level == self.root_level() {
            let (page_nr, branch) = Branch::allocate(lock);
            branch.children_mut()[0] = *self.root;
            branch.insert_right(0, key, value);
            *self.root = page_nr;
            self.entries.push(Entry { page_nr, index: 0 });
            return;
        }
        let index = self.entries[level + 1].index;
        let branch = self.branch_mut(level + 1, lock);
        if branch.len() < Branch::<K>::order() {
            branch.insert_right(index, key, value);
        } else {
            let (page_nr, other) = Branch::allocate(lock);
            let split_key = branch.split(other);
            let len = branch.len();
            self.parent_insert(level + 1, split_key, page_nr, lock);
            if index < len {
                branch.insert_right(index, key, value);
            } else {
                let index = index - len;
                other.insert_right(index, key, value);
                self.entries[level + 1] = Entry { page_nr, index };
            }
        }
    }

    pub unsafe fn delete(mut self, lock: &'a Lock) -> bool {
        if self.entries.is_empty() {
            false
        } else if self.key.is_none() {
            let index = self.entries[0].index;
            let height = self.height();
            let leaf = self.leaf_mut(lock);
            leaf.delete(index);
            if height > 1 && leaf.len() * 2 < Leaf::<K, V>::order() {
                self.rebalance(0, lock)
            } else if height == 1 && leaf.len() == 0 {
                lock.deallocate(*self.root);
                self.entries.clear();
                *self.root = NULL_PAGE_NR;
            }
            true
        } else {
            false
        }
    }

    unsafe fn rebalance(&mut self, level: usize, lock: &'a Lock) {
        let index = self.entries[level + 1].index;
        let branch = self.branch_mut(level + 1, lock);
        if index > 0 {
            let child = &mut branch.children_mut()[index - 1];
            if level == 0 {
                let left_leaf = Leaf::wrap_mut(lock.page_mut(child));
                if left_leaf.len() * 2 > Leaf::<K, V>::order() {
                    let key = left_leaf.shift_right(self.leaf_mut(lock));
                    branch.keys_mut()[index - 1] = key;
                    return;
                }
            } else {
                let left_branch = Branch::wrap_mut(lock.page_mut(child));
                if left_branch.len() * 2 > Branch::<K>::order() {
                    let key = left_branch.shift_right::<V>(self.branch_mut(level, lock), lock);
                    branch.keys_mut()[index - 1] = key;
                    return;
                }
            }
        }
        if index + 1 < branch.len() {
            let child = &mut branch.children_mut()[index + 1];
            if level == 0 {
                let right_leaf = Leaf::wrap_mut(lock.page_mut(child));
                if right_leaf.len() * 2 > Leaf::<K, V>::order() {
                    let key = self.leaf_mut(lock).shift_left(right_leaf);
                    branch.keys_mut()[index] = key;
                    return;
                }
            } else {
                let right_branch = Branch::wrap_mut(lock.page_mut(child));
                if right_branch.len() * 2 > Branch::<K>::order() {
                    let key = self
                        .branch_mut(level, lock)
                        .shift_left::<V>(right_branch, lock);
                    branch.keys_mut()[index] = key;
                    return;
                }
            }
        }
        if index > 0 {
            let child = &mut branch.children_mut()[index - 1];
            if level == 0 {
                let left_leaf = Leaf::<K, V>::wrap_mut(lock.page_mut(child));
                let child = *child;
                left_leaf.merge(self.leaf_mut(lock));
                branch.delete_left(index);
                self.entries[level + 1].index -= 1;
                lock.deallocate(child);
            } else {
                let left_branch = Branch::wrap_mut(lock.page_mut(child));
                let child = *child;
                left_branch.merge::<V>(self.branch_mut(level, lock), lock);
                branch.delete_left(index);
                self.entries[level + 1].index -= 1;
                lock.deallocate(child);
            }
        } else {
            let child = &mut branch.children_mut()[index + 1];
            if level == 0 {
                let right_leaf = Leaf::wrap_mut(lock.page_mut(child));
                self.leaf_mut(lock).merge(right_leaf);
            } else {
                let right_branch = Branch::wrap_mut(lock.page_mut(child));
                self.branch_mut(level, lock).merge::<V>(right_branch, lock);
            }
            branch.delete_right(index);
            lock.deallocate(self.entries[level + 1].page_nr);
        }
        if level + 1 == self.root_level() && branch.len() == 1 {
            *self.root = branch.children()[0];
            self.entries.pop();
        } else if branch.len() * 2 < Branch::<K>::order() {
            self.rebalance(level + 1, lock);
        }
    }
}
