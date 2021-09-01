use std::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use bytemuck::{Pod, Zeroable};
use memmap2::MmapRaw;

use crate::mmap::{MappedBitset, MappedFile, MappedVec};

pub const PAGE_SIZE: usize = 8192;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Page([u8; PAGE_SIZE]);

unsafe impl Pod for Page {}
unsafe impl Zeroable for Page {}

impl Deref for Page {
    type Target = [u8; PAGE_SIZE];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Page {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub type PageNr = u32;

pub const NULL_PAGE_NR: PageNr = 0;

pub struct Pager {
    inner: UnsafeCell<Inner>,
}

pub struct Inner {
    pages: MappedVec<Page>,
    old_raws: Vec<Arc<MmapRaw>>,
    writable: MappedBitset,
}

impl Pager {
    pub fn new(data: MappedFile, writable: MappedBitset) -> Self {
        let pages = MappedVec::new(data).unwrap();
        Self {
            inner: UnsafeCell::new(Inner {
                pages,
                old_raws: Vec::new(),
                writable,
            }),
        }
    }

    pub unsafe fn page(&self, nr: PageNr) -> &Page {
        let inner = &mut *self.inner.get();
        let index = nr as usize;
        inner.grow(index + 1);
        &*inner.pages.get(index).get()
    }

    #[allow(clippy::mut_from_ref)]
    pub unsafe fn page_mut(&self, nr: PageNr) -> &mut Page {
        let inner = &mut *self.inner.get();
        let index = nr as usize;
        inner.grow(index + 1);
        &mut *inner.pages.get(index).get()
    }

    #[allow(clippy::mut_from_ref)]
    pub unsafe fn copy_page_mut(&self, from: PageNr, to: PageNr) -> &mut Page {
        let inner = &mut *self.inner.get();
        let from_index = from as usize;
        let to_index = to as usize;
        inner.grow(from_index.max(to_index) + 1);
        let to_ptr = inner.pages.get(to_index).get();
        to_ptr.copy_from_nonoverlapping(inner.pages.get(from_index).get(), 1);
        &mut *to_ptr
    }

    pub unsafe fn enable_write(&self, nr: PageNr) {
        let inner = &mut *self.inner.get();
        let index = nr as usize;
        inner.grow(index + 1);
        inner.writable.set(index)
    }

    pub unsafe fn can_write(&self, nr: PageNr) -> bool {
        let inner = &mut *self.inner.get();
        let index = nr as usize;
        inner.grow(index + 1);
        inner.writable.get(index)
    }
}

impl Inner {
    unsafe fn grow(&mut self, min_len: usize) {
        if min_len > self.pages.len() {
            self.old_raws.push(self.pages.grow(min_len).unwrap())
        }
        if min_len > self.writable.len() {
            self.old_raws.push(self.writable.grow(min_len).unwrap())
        }
    }
}
