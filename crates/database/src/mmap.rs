use std::{
    cell::UnsafeCell,
    fs::File,
    io,
    marker::PhantomData,
    mem::{size_of, swap},
    ops::DerefMut,
    sync::{Arc, Mutex},
};

use bytemuck::Pod;
use memmap2::MmapRaw;

#[derive(Clone)]
pub struct MappedFile(Arc<(Mutex<Arc<MmapRaw>>, File)>);

impl MappedFile {
    pub fn new(file: File) -> io::Result<Self> {
        let page_size = page_size::get() as u64;
        if file.metadata()?.len() < page_size {
            file.set_len(page_size)?;
        }
        let raw = Arc::new(MmapRaw::map_raw(&file)?);
        Ok(Self(Arc::new((Mutex::new(raw), file))))
    }

    fn raw(&self, min_len: usize) -> io::Result<Arc<MmapRaw>> {
        let mut raw = self.0 .0.lock().unwrap();
        let len = raw.len();
        if len < min_len {
            self.0 .1.set_len(min_len.max(len * 2) as u64)?;
            *raw.deref_mut() = Arc::new(MmapRaw::map_raw(&self.0 .1)?)
        }
        Ok(raw.clone())
    }

    pub fn sync(&self) -> io::Result<()> {
        self.0 .1.sync_data()
    }

    pub fn len(&self) -> usize {
        self.0 .0.lock().unwrap().len()
    }
}

#[derive(Clone)]
struct MappedBuffer {
    raw: Arc<MmapRaw>,
    file: MappedFile,
}

impl MappedBuffer {
    fn new(file: MappedFile) -> io::Result<Self> {
        let raw = file.raw(0)?;
        Ok(Self { raw, file })
    }

    fn grow(&mut self, min_len: usize) -> io::Result<Arc<MmapRaw>> {
        Ok(if self.len() < min_len {
            let mut raw = self.file.raw(min_len)?;
            swap(&mut self.raw, &mut raw);
            raw
        } else {
            self.raw.clone()
        })
    }

    fn len(&self) -> usize {
        self.raw.len()
    }

    fn as_ptr(&self) -> *const u8 {
        self.raw.as_ptr()
    }
}

pub struct MappedVec<T: Pod>(MappedBuffer, PhantomData<T>);

impl<T: Pod> MappedVec<T> {
    pub fn new(file: MappedFile) -> io::Result<Self> {
        Ok(Self(MappedBuffer::new(file)?, PhantomData))
    }

    pub fn get(&self, index: usize) -> &UnsafeCell<T> {
        assert!(index < self.len());
        unsafe {
            self.0
                .as_ptr()
                .cast::<UnsafeCell<T>>()
                .add(index)
                .as_ref()
                .unwrap()
        }
    }

    pub fn len(&self) -> usize {
        self.0.len() / size_of::<T>()
    }

    pub fn grow(&mut self, min_len: usize) -> io::Result<Arc<MmapRaw>> {
        self.0.grow(min_len * size_of::<T>())
    }
}

impl<T: Pod> Clone for MappedVec<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

#[derive(Clone)]
pub struct MappedBitset(MappedVec<u8>);

impl MappedBitset {
    pub fn new(len: usize) -> io::Result<Self> {
        let file = tempfile::tempfile()?;
        file.set_len(len as u64)?;
        Ok(Self(MappedVec::new(MappedFile::new(file)?)?))
    }

    pub unsafe fn get(&mut self, index: usize) -> bool {
        *self.0.get(index).get() != 0
    }

    pub unsafe fn set(&mut self, index: usize) {
        *self.0.get(index).get() = 1;
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn grow(&mut self, min_len: usize) -> io::Result<Arc<MmapRaw>> {
        self.0.grow(min_len)
    }
}
