use std::{
    marker::PhantomData,
    mem::{align_of, size_of},
};

use bytemuck::{cast_slice, cast_slice_mut, Pod, TransparentWrapper};

use crate::{
    lock::Lock,
    page::{Page, PageNr, PAGE_SIZE},
};

use super::node::{allocate, NodePage};

#[derive(Clone, Copy, TransparentWrapper)]
#[repr(transparent)]
#[transparent(Page)]
pub struct Leaf<K, V> {
    page: Page,
    _phantom_key: PhantomData<K>,
    _phantom_value: PhantomData<V>,
}

impl<K, V> Leaf<K, V> {
    pub const fn order() -> usize {
        let value_align = align_of::<V>();
        let key_size = size_of::<K>();
        let sum_size = key_size + size_of::<V>();
        let order = (PAGE_SIZE - 2) / sum_size;
        let mismatch = (order * key_size) % value_align;
        if mismatch == 0 {
            order
        } else {
            let offset = value_align - mismatch;
            if order * sum_size + offset <= PAGE_SIZE - 2 {
                order
            } else {
                order - 1
            }
        }
    }

    const fn value_offset() -> usize {
        let key_size = size_of::<K>();
        let value_align = align_of::<V>();
        let order = Self::order();
        let mismatch = (order * key_size) % value_align;
        if mismatch == 0 {
            order * key_size
        } else {
            order * key_size + value_align - mismatch
        }
    }
}

impl<K: Pod + Ord, V: Pod> Leaf<K, V> {
    pub unsafe fn allocate<'a>(lock: &'a Lock) -> (PageNr, &'a mut Self) {
        let (page_nr, page) = allocate(lock, true);
        (page_nr, Self::wrap_mut(page))
    }

    pub fn search(&self, key: &K) -> Result<usize, usize> {
        self.keys().binary_search(key)
    }

    pub fn len(&self) -> usize {
        self.page.len()
    }

    pub fn keys(&self) -> &[K] {
        cast_slice(&self.page[0..self.page.len() * size_of::<K>()])
    }

    pub fn keys_mut(&mut self) -> &mut [K] {
        let len = self.page.len();
        cast_slice_mut(&mut self.page[0..len * size_of::<K>()])
    }

    pub fn values(&self) -> &[V] {
        let offset = Self::value_offset();
        cast_slice(&self.page[offset..offset + self.page.len() * size_of::<V>()])
    }

    pub fn values_mut(&mut self) -> &mut [V] {
        let offset = Self::value_offset();
        let len = self.page.len();
        cast_slice_mut(&mut self.page[offset..offset + len * size_of::<V>()])
    }

    pub fn split(&mut self, other: &mut Self) -> K {
        let order = Self::order();
        let mid = (order + 1) / 2;
        other.page.set_len(order - mid);
        other.keys_mut().copy_from_slice(&self.keys()[mid..]);
        other.values_mut().copy_from_slice(&self.values()[mid..]);
        self.page.set_len(mid);
        other.keys()[0]
    }

    pub fn insert(&mut self, index: usize, key: K, value: V) {
        self.page.set_len(self.len() + 1);
        let keys = self.keys_mut();
        keys.copy_within(index..keys.len() - 1, index + 1);
        keys[index] = key;
        let values = self.values_mut();
        values.copy_within(index..values.len() - 1, index + 1);
        values[index] = value;
    }

    pub fn delete(&mut self, index: usize) {
        let keys = self.keys_mut();
        keys.copy_within(index + 1.., index);
        let values = self.values_mut();
        values.copy_within(index + 1.., index);
        self.page.set_len(self.page.len() - 1);
    }

    pub unsafe fn shift_left(&mut self, right: &mut Self) -> K {
        self.insert(self.page.len() - 1, right.keys()[0], right.values()[0]);
        right.delete(0);
        right.keys()[0]
    }

    pub unsafe fn shift_right(&mut self, right: &mut Self) -> K {
        let index = self.page.len() - 1;
        let key = self.keys()[index];
        right.insert(0, self.keys()[index], self.values()[index]);
        self.page.set_len(index);
        key
    }

    pub unsafe fn merge(&mut self, right: &mut Self) {
        let len = self.page.len();
        self.page.set_len(len + right.page.len());
        self.keys_mut()[len..].copy_from_slice(right.keys());
        self.values_mut()[len..].copy_from_slice(right.values());
    }
}
