mod branch;
mod cursor;
mod leaf;
mod node;

use std::{
    io::{self, Read, Write},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use bytemuck::Pod;

use crate::{
    lock::Lock,
    page::{PageNr, NULL_PAGE_NR},
    reference::DatabaseRef,
};

use self::{cursor::Cursor, node::NodeRef};

pub struct Tree<K: Pod + Ord, V: Pod> {
    root: PageNr,
    database: DatabaseRef,
    _phantom_key: PhantomData<K>,
    _phantom_value: PhantomData<V>,
}

impl<K: Pod + Ord, V: Pod> Tree<K, V> {
    pub fn new(database: DatabaseRef) -> Self {
        Self {
            root: NULL_PAGE_NR,
            database,
            _phantom_key: PhantomData,
            _phantom_value: PhantomData,
        }
    }

    pub fn deserialize(reader: &mut impl Read, database: DatabaseRef) -> io::Result<Self> {
        let mut bytes = [0; 4];
        reader.read_exact(&mut bytes)?;
        let root = u32::from_ne_bytes(bytes);
        Ok(Self {
            root,
            database,
            _phantom_key: PhantomData,
            _phantom_value: PhantomData,
        })
    }

    pub fn serialize(&self, writer: &mut impl Write) -> io::Result<()> {
        writer.write_all(&self.root.to_ne_bytes())?;
        Ok(())
    }

    pub fn read(&self) -> ReadTreeGuard<'_, K, V> {
        ReadTreeGuard {
            root: &self.root,
            lock: self.database.lock(),
            _phantom_key: PhantomData,
            _phantom_value: PhantomData,
        }
    }

    pub fn write(&mut self) -> WriteTreeGuard<'_, K, V> {
        WriteTreeGuard {
            root: &mut self.root,
            lock: self.database.lock(),
            _phantom_key: PhantomData,
            _phantom_value: PhantomData,
        }
    }
}

impl<'a, R: DerefMut<Target = PageNr>, K: Pod + Ord, V: Pod> TreeGuard<'a, R, K, V> {
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let mut cursor = Cursor::new(self.root.deref_mut(), &key, &self.lock);
        let old = cursor.value(&self.lock).cloned();
        unsafe { cursor.set_value(value, &self.lock) };
        old
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let cursor = Cursor::new(self.root.deref_mut(), key, &self.lock);
        let old = cursor.value(&self.lock).cloned();
        unsafe { cursor.delete(&self.lock) };
        old
    }

    pub fn entry<'b>(&mut self, key: &'b K) -> Entry<'b, '_, 'a, &mut PageNr, K, V> {
        let cursor = Cursor::new(self.root.deref_mut(), key, &self.lock);
        if cursor.has_value() {
            Entry::Occupied(OccupiedEntry {
                _cursor: cursor,
                _lock: &self.lock,
            })
        } else {
            Entry::Vacant(VacantEntry {
                cursor,
                lock: &self.lock,
            })
        }
    }

    pub fn clear(&mut self) {
        if *self.root == NULL_PAGE_NR {
            return;
        }
        unsafe {
            if let NodeRef::Branch(branch) = node::node::<K, V>(self.lock.page(*self.root)) {
                branch.deallocate_children::<V>(&self.lock)
            }
            self.lock.deallocate(*self.root);
        }
        *self.root = NULL_PAGE_NR;
    }
}

impl<'a, R: Deref<Target = PageNr>, K: Pod + Ord, V: Pod> TreeGuard<'a, R, K, V> {
    pub fn get(&self, key: &K) -> Option<&V> {
        Cursor::new(self.root.deref(), key, &self.lock).value(&self.lock)
    }
}

pub struct TreeGuard<'a, R: Deref<Target = PageNr>, K: Pod + Ord, V: Pod> {
    root: R,
    lock: Lock<'a>,
    _phantom_key: PhantomData<K>,
    _phantom_value: PhantomData<V>,
}

pub type ReadTreeGuard<'a, K, V> = TreeGuard<'a, &'a PageNr, K, V>;
pub type WriteTreeGuard<'a, K, V> = TreeGuard<'a, &'a mut PageNr, K, V>;

impl<K: Pod + Ord, V: Pod> Drop for Tree<K, V> {
    fn drop(&mut self) {
        let lock = self.database.lock();
        if !lock.is_closing() {
            drop(lock);
            self.write().clear();
        }
    }
}

pub enum Entry<'a, 'b, 'c, R: DerefMut<Target = PageNr>, K: 'a + Pod + Ord, V: 'a + Pod> {
    Occupied(OccupiedEntry<'a, 'b, 'c, R, K, V>),
    Vacant(VacantEntry<'a, 'b, 'c, R, K, V>),
}

pub struct OccupiedEntry<'a, 'b, 'c, R: DerefMut<Target = PageNr>, K: Pod + Ord, V: Pod> {
    _cursor: Cursor<'a, R, K, V>,
    _lock: &'b Lock<'c>,
}

pub struct VacantEntry<'a, 'b: 'a, 'c, R: DerefMut<Target = PageNr>, K: Pod + Ord, V: Pod> {
    cursor: Cursor<'a, R, K, V>,
    lock: &'b Lock<'c>,
}

impl<'a, 'b: 'a, 'c, R: DerefMut<Target = PageNr>, K: Pod + Ord, V: Pod>
    VacantEntry<'a, 'b, 'c, R, K, V>
{
    pub fn insert(mut self, value: V) -> &'b mut V {
        unsafe {
            self.cursor.set_value(value, self.lock);
            self.cursor.value_mut(self.lock).unwrap()
        }
    }
}
