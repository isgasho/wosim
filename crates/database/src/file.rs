use std::{
    io::{self, Read, Seek, Write},
    ops::{Deref, DerefMut},
};

use crate::{
    cursor::{reallocate, PageLookup},
    lock::Lock,
    page::{PageNr, PAGE_SIZE},
    reference::DatabaseRef,
};

#[derive(Clone, Default, Copy)]
#[repr(C)]
pub struct FileHeader {
    pub root: PageNr,
    pub len: u64,
}

impl FileHeader {
    fn pages(&self) -> usize {
        (self.len as usize + PAGE_SIZE - 1) / PAGE_SIZE
    }
}

pub struct File {
    header: FileHeader,
    database: DatabaseRef,
}

impl File {
    pub fn new(database: DatabaseRef) -> Self {
        Self {
            header: FileHeader::default(),
            database,
        }
    }

    pub(crate) fn from_header(header: FileHeader, database: DatabaseRef) -> Self {
        Self { header, database }
    }

    pub fn deserialize(reader: &mut impl Read, database: DatabaseRef) -> io::Result<Self> {
        let mut bytes = [0; 4];
        reader.read_exact(&mut bytes)?;
        let root = u32::from_ne_bytes(bytes);
        let mut bytes = [0; 8];
        reader.read_exact(&mut bytes)?;
        let len = u64::from_ne_bytes(bytes);
        Ok(Self {
            header: FileHeader { root, len },
            database,
        })
    }

    pub fn serialize(&self, writer: &mut impl Write) -> io::Result<()> {
        writer.write_all(&self.header.root.to_ne_bytes())?;
        writer.write_all(&self.header.len.to_ne_bytes())?;
        Ok(())
    }

    pub(crate) fn header(&self) -> FileHeader {
        self.header
    }

    pub fn read(&self) -> ReadFileGuard<'_> {
        ReadFileGuard {
            header: &self.header,
            lock: self.database.lock(),
            pos: 0,
            lookup: PageLookup::Invalid,
        }
    }

    pub fn write(&mut self) -> WriteFileGuard<'_> {
        WriteFileGuard {
            header: &mut self.header,
            lock: self.database.lock(),
            pos: 0,
            lookup: PageLookup::Invalid,
        }
    }
}

impl<'a, H: DerefMut<Target = FileHeader>> FileGuard<'a, H> {
    pub fn set_len(&mut self, size: u64) {
        let current_pages = self.header.pages();
        self.header.len = size;
        let new_pages = self.header.pages();
        reallocate(&mut self.header.root, current_pages, new_pages, &self.lock);
        self.lookup = PageLookup::Invalid;
    }
}

impl<'a, H: Deref<Target = FileHeader>> Seek for FileGuard<'a, H> {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        match pos {
            io::SeekFrom::Start(pos) => self.pos = pos,
            io::SeekFrom::End(offset) => self.pos = (self.header.len as i64 + offset) as u64,
            io::SeekFrom::Current(offset) => self.pos = ((self.pos as i64) + offset) as u64,
        }
        Ok(self.pos)
    }
}

impl<'a, H: Deref<Target = FileHeader>> Read for FileGuard<'a, H> {
    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        let len = buf
            .len()
            .min((self.header.len as usize).saturating_sub(self.pos as usize));
        buf = buf.split_at_mut(len).0;
        while !buf.is_empty() {
            let index = self.pos as usize / PAGE_SIZE;
            let offset = self.pos as usize % PAGE_SIZE;
            let n = buf.len().min(PAGE_SIZE - offset);
            let (a, b) = buf.split_at_mut(n);
            let page = self
                .lookup
                .get(self.header.root, self.header.pages(), index, &self.lock);
            a.copy_from_slice(&page[offset..offset + n]);
            self.pos += n as u64;
            buf = b;
        }
        Ok(len)
    }
}

impl<'a, H: DerefMut<Target = FileHeader>> Write for FileGuard<'a, H> {
    fn write(&mut self, mut buf: &[u8]) -> io::Result<usize> {
        let len = buf.len();
        let size = self.pos + buf.len() as u64;
        if size > self.header.len as u64 {
            self.set_len(size)
        }
        let pages = self.header.pages();
        while !buf.is_empty() {
            let index = self.pos as usize / PAGE_SIZE;
            let offset = self.pos as usize % PAGE_SIZE;
            let n = buf.len().min(PAGE_SIZE - offset);
            let (a, b) = buf.split_at(n);
            let page = unsafe {
                self.lookup
                    .get_mut(&mut self.header.root, pages, index, &self.lock)
                    .as_mut()
                    .unwrap()
            };
            page[offset..offset + n].copy_from_slice(a);
            self.pos += n as u64;
            buf = b;
        }
        Ok(len)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct FileGuard<'a, H: Deref<Target = FileHeader>> {
    header: H,
    pos: u64,
    lookup: PageLookup,
    lock: Lock<'a>,
}

pub type ReadFileGuard<'a> = FileGuard<'a, &'a FileHeader>;
pub type WriteFileGuard<'a> = FileGuard<'a, &'a mut FileHeader>;

impl Drop for File {
    fn drop(&mut self) {
        let lock = self.database.lock();
        if !lock.is_closing() {
            drop(lock);
            self.write().set_len(0);
        }
    }
}
