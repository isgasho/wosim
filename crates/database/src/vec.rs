use std::{
    cell::Cell,
    intrinsics::transmute,
    io::{self, Read, Write},
    marker::PhantomData,
    mem::size_of,
    ops::{Deref, DerefMut, Index, IndexMut},
};

use bytemuck::{cast_slice, cast_slice_mut, Pod};

use crate::{
    cursor::{reallocate, PageLookup},
    lock::Lock,
    page::{PageNr, PAGE_SIZE},
    reference::DatabaseRef,
};

#[derive(Clone, Default, Copy)]
#[repr(C)]
pub struct VecHeader {
    root: PageNr,
    len: usize,
}

impl VecHeader {
    fn pages<T>(&self) -> usize {
        (self.len + elements_per_page::<T>() - 1) / elements_per_page::<T>()
    }
}

const fn elements_per_page<T>() -> usize {
    PAGE_SIZE / size_of::<T>()
}

pub struct Vec<T: Pod> {
    header: VecHeader,
    database: DatabaseRef,
    _phantom: PhantomData<T>,
}

pub struct VecGuard<'a, T: Pod, H> {
    header: H,
    lock: Lock<'a>,
    lookup: Cell<PageLookup>,
    _phantom: PhantomData<T>,
}

pub type ReadVecGuard<'a, T> = VecGuard<'a, T, &'a VecHeader>;
pub type WriteVecGuard<'a, T> = VecGuard<'a, T, &'a mut VecHeader>;

impl<T: Pod> Vec<T> {
    pub fn new(database: DatabaseRef) -> Self {
        Self {
            header: VecHeader::default(),
            database,
            _phantom: PhantomData,
        }
    }

    pub fn write(&mut self) -> WriteVecGuard<'_, T> {
        WriteVecGuard {
            header: &mut self.header,
            lock: self.database.lock(),
            lookup: Cell::new(PageLookup::Invalid),
            _phantom: PhantomData,
        }
    }

    pub fn read(&self) -> ReadVecGuard<'_, T> {
        ReadVecGuard {
            header: &self.header,
            lock: self.database.lock(),
            lookup: Cell::new(PageLookup::Invalid),
            _phantom: PhantomData,
        }
    }

    pub fn deserialize(reader: &mut impl Read, database: DatabaseRef) -> io::Result<Self> {
        let mut bytes = [0; 4];
        reader.read_exact(&mut bytes)?;
        let root = u32::from_ne_bytes(bytes);
        let mut bytes = [0; 8];
        reader.read_exact(&mut bytes)?;
        let len = u64::from_ne_bytes(bytes) as usize;
        Ok(Self {
            header: VecHeader { root, len },
            database,
            _phantom: PhantomData,
        })
    }

    pub fn serialize(&self, writer: &mut impl Write) -> io::Result<()> {
        writer.write_all(&self.header.root.to_ne_bytes())?;
        writer.write_all(&(self.header.len as u64).to_ne_bytes())?;
        Ok(())
    }
}

impl<T: Pod> Drop for Vec<T> {
    fn drop(&mut self) {
        let lock = self.database.lock();
        if !lock.is_closing() {
            drop(lock);
            self.write().internal_resize(0)
        }
    }
}

impl<'a, T: Pod, H: Deref<Target = VecHeader>> VecGuard<'a, T, H> {
    pub fn iter(&self) -> Iter<'_, Self> {
        Iter {
            container: self,
            pos: 0,
        }
    }
}

impl<'a, T: Pod, H: Deref<Target = VecHeader>> Len for VecGuard<'a, T, H> {
    fn len(&self) -> usize {
        self.header.len
    }
}

impl<'a, T: Pod, H: DerefMut<Target = VecHeader>> VecGuard<'a, T, H> {
    fn internal_resize(&mut self, new_len: usize) {
        let current_pages = self.header.pages::<T>();
        self.header.len = new_len;
        let new_pages = self.header.pages::<T>();
        reallocate(&mut self.header.root, current_pages, new_pages, &self.lock)
    }

    pub fn resize(&mut self, new_len: usize, value: T) {
        let old_len = self.header.len;
        self.internal_resize(new_len);
        let mut i = old_len;
        while i != new_len {
            self[i] = value;
            i += 1;
        }
    }

    pub fn push(&mut self, value: T) {
        let index = self.header.len;
        self.internal_resize(index + 1);
        self[index] = value;
    }

    pub fn append(&mut self, values: &[T]) {
        let mut index = self.header.len;
        self.internal_resize(index + values.len());
        for value in values {
            self[index] = *value;
            index += 1;
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.header.len > 0 {
            let value = self[self.header.len - 1];
            self.internal_resize(self.header.len - 1);
            Some(value)
        } else {
            None
        }
    }

    pub fn copy_within(&mut self, src: usize, dest: usize) {
        self[dest] = self[src]
    }
}

impl<'a, T: Pod, H: Deref<Target = VecHeader>> Index<usize> for VecGuard<'a, T, H> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.header.len);
        let page_index = index / elements_per_page::<T>();
        let page_offset = index % elements_per_page::<T>() * size_of::<T>();
        let pages = self.header.pages::<T>();
        let mut lookup = self.lookup.get();
        let page = lookup.get(self.header.root, pages, page_index, &self.lock);
        self.lookup.set(lookup);
        &cast_slice::<u8, T>(&page[page_offset..page_offset + size_of::<T>()])[0]
    }
}

impl<'a, T: Pod, H: DerefMut<Target = VecHeader>> IndexMut<usize> for VecGuard<'a, T, H> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(index < self.header.len);
        let page_index = index / elements_per_page::<T>();
        let page_offset = index % elements_per_page::<T>() * size_of::<T>();
        let pages = self.header.pages::<T>();
        let page = unsafe {
            self.lookup
                .get_mut()
                .get_mut(&mut self.header.root, pages, page_index, &self.lock)
                .as_mut()
                .unwrap()
        };
        &mut cast_slice_mut::<u8, T>(&mut page[page_offset..page_offset + size_of::<T>()])[0]
    }
}

pub trait Len {
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub struct Iter<'a, T> {
    container: &'a T,
    pos: usize,
}

struct IterMut<'a, T> {
    container: &'a mut T,
    pos: usize,
}

impl<'a, T: Index<usize> + Len> Iterator for Iter<'a, T> {
    type Item = &'a T::Output;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.container.len() {
            let index = self.pos;
            self.pos += 1;
            Some(self.container.index(index))
        } else {
            None
        }
    }
}

impl<'a, T: IndexMut<usize> + Len> Iterator for IterMut<'a, T> {
    type Item = &'a mut T::Output;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.container.len() {
            let index = self.pos;
            self.pos += 1;
            Some(unsafe { transmute(self.container.index_mut(index)) })
        } else {
            None
        }
    }
}
