use std::{marker::PhantomData, mem::size_of};

use bytemuck::{cast_slice, cast_slice_mut, Pod, TransparentWrapper};

use crate::{
    lock::Lock,
    page::{Page, PageNr, PAGE_SIZE},
    tree::node::NodeRef,
};

use super::node::{allocate, node, NodePage};

#[derive(Clone, Copy, TransparentWrapper)]
#[repr(transparent)]
#[transparent(Page)]
pub struct Branch<K> {
    page: Page,
    _phantom_key: PhantomData<K>,
}

impl<K> Branch<K> {
    const fn key_order() -> usize {
        let child_align = 4;
        let key_size = size_of::<K>();
        let sum_size = key_size + 4;
        let order = (PAGE_SIZE - 6) / sum_size;
        let mismatch = (order * key_size) % child_align;
        if mismatch == 0 {
            order
        } else {
            let offset = child_align - mismatch;
            if order * sum_size + offset <= PAGE_SIZE - 6 {
                order
            } else {
                order - 1
            }
        }
    }

    pub const fn order() -> usize {
        Self::key_order() + 1
    }

    const fn child_offset() -> usize {
        let key_size = size_of::<K>();
        let child_align = 4;
        let key_order = Self::key_order();
        let mismatch = (key_order * key_size) % child_align;
        if mismatch == 0 {
            key_order * key_size
        } else {
            key_order * key_size + child_align - mismatch
        }
    }
}

impl<K: Pod + Ord> Branch<K> {
    pub unsafe fn allocate<'a>(lock: &'a Lock) -> (PageNr, &'a mut Self) {
        let (page_nr, page) = allocate(lock, false);
        (page_nr, Self::wrap_mut(page))
    }

    pub fn keys(&self) -> &[K] {
        cast_slice(&self.page[0..(self.page.len() - 1) * size_of::<K>()])
    }

    pub fn keys_mut(&mut self) -> &mut [K] {
        let len = self.page.len();
        cast_slice_mut(&mut self.page[0..(len - 1) * size_of::<K>()])
    }

    pub fn children(&self) -> &[PageNr] {
        let offset = Self::child_offset();
        cast_slice(&self.page[offset..offset + self.page.len() * size_of::<PageNr>()])
    }

    pub fn children_mut(&mut self) -> &mut [PageNr] {
        let offset = Self::child_offset();
        let len = self.page.len();
        cast_slice_mut(&mut self.page[offset..offset + len * size_of::<PageNr>()])
    }

    pub fn split(&mut self, other: &mut Self) -> K {
        let order = Self::order();
        let mid = (order + 1) / 2;
        other.page.set_len(order - mid);
        other.keys_mut().copy_from_slice(&self.keys()[mid..]);
        other
            .children_mut()
            .copy_from_slice(&self.children()[mid..]);
        let key = self.keys()[mid - 1];
        self.page.set_len(mid);
        key
    }

    pub fn search(&self, key: &K) -> usize {
        match self.keys().binary_search(key) {
            Ok(index) => index + 1,
            Err(index) => index,
        }
    }

    pub fn len(&self) -> usize {
        self.page.len()
    }

    pub fn insert_left(&mut self, key_index: usize, key: K, child: PageNr) {
        self.page.set_len(self.page.len() + 1);
        let keys = self.keys_mut();
        keys.copy_within(key_index..keys.len() - 1, key_index + 1);
        keys[key_index] = key;
        let children = self.children_mut();
        children.copy_within(key_index..children.len() - 1, key_index + 1);
        children[key_index] = child;
    }

    pub fn insert_right(&mut self, key_index: usize, key: K, child: PageNr) {
        self.page.set_len(self.page.len() + 1);
        let keys = self.keys_mut();
        keys.copy_within(key_index..keys.len() - 1, key_index + 1);
        keys[key_index] = key;
        let children = self.children_mut();
        children.copy_within(key_index + 1..children.len() - 1, key_index + 2);
        children[key_index + 1] = child;
    }

    pub fn delete_left(&mut self, key_index: usize) {
        let keys = self.keys_mut();
        keys.copy_within(key_index + 1.., key_index);
        let children = self.children_mut();
        children.copy_within(key_index + 1.., key_index);
        self.page.set_len(self.page.len() - 1);
    }

    pub fn delete_right(&mut self, key_index: usize) {
        let keys = self.keys_mut();
        keys.copy_within(key_index + 1.., key_index);
        let children = self.children_mut();
        children.copy_within(key_index + 2.., key_index + 1);
        self.page.set_len(self.page.len() - 1);
    }

    pub unsafe fn left_key<'a, V: Pod>(&self, lock: &'a Lock) -> K {
        let page = lock.page(self.children()[0]);
        match node::<K, V>(page) {
            NodeRef::Branch(branch) => branch.left_key::<V>(lock),
            NodeRef::Leaf(leaf) => leaf.keys()[0],
        }
    }

    pub unsafe fn shift_left<'a, V: Pod>(&mut self, right: &mut Self, lock: &'a Lock) -> K {
        let left_key = right.left_key::<V>(lock);
        let key = right.keys()[0];
        self.insert_right(self.page.len(), left_key, right.children()[0]);
        right.delete_left(0);
        key
    }

    pub unsafe fn shift_right<'a, V: Pod>(&mut self, right: &mut Self, lock: &'a Lock) -> K {
        let left_key = right.left_key::<V>(lock);
        let index = self.page.len() - 1;
        let key = self.keys()[index];
        right.insert_left(0, left_key, self.children()[index - 1]);
        self.delete_right(index);
        key
    }

    pub unsafe fn merge<'a, V: Pod>(&mut self, right: &mut Self, lock: &'a Lock) {
        let key = right.left_key::<V>(lock);
        let len = self.page.len();
        self.page.set_len(len + right.page.len());
        self.keys_mut()[len - 1] = key;
        self.keys_mut()[len..].copy_from_slice(right.keys());
        self.children_mut()[len..].copy_from_slice(right.children());
    }

    pub unsafe fn deallocate_children<'a, V: Pod>(&self, lock: &'a Lock) {
        for page_nr in self.children() {
            if let NodeRef::Branch(branch) = node::<K, V>(lock.page(*page_nr)) {
                branch.deallocate_children::<V>(lock)
            }
            lock.deallocate(*page_nr)
        }
    }
}
