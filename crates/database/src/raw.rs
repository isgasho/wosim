use std::{
    fs::File,
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, MutexGuard,
    },
};

use bytemuck::cast_mut;

use crate::{
    allocator::AllocatorState,
    file::FileHeader,
    header::{Format, Header, HeaderPage, State},
    mmap::{MappedBitset, MappedFile},
    page::{Page, Pager, NULL_PAGE_NR, PAGE_SIZE},
    sync::Synchronizer,
};

pub struct RawDatabase {
    allocator_state: Mutex<AllocatorState>,
    version: u64,
    data: MappedFile,
    writable: MappedBitset,
    synchronizer: Synchronizer,
    closing: AtomicBool,
}

impl RawDatabase {
    fn new(
        file: File,
        format: &Format,
        setup_header: impl FnOnce(&mut Header),
    ) -> io::Result<(Self, FileHeader)> {
        let data = MappedFile::new(file)?;
        let writable = MappedBitset::new(data.len() / PAGE_SIZE)?;
        let pager = Pager::new(data.clone(), writable.clone());
        let page = unsafe { pager.page_mut(NULL_PAGE_NR) };
        let header_page = cast_mut::<Page, HeaderPage>(page);
        setup_header(&mut header_page.header);
        let state = header_page.header.validate(format)?;
        Ok((
            Self {
                allocator_state: Mutex::new(state.allocator),
                version: state.version,
                synchronizer: Synchronizer::new(data.clone()),
                data,
                writable,
                closing: AtomicBool::new(false),
            },
            FileHeader {
                root: state.root_nr,
                len: state.root_len,
            },
        ))
    }

    pub fn create(file: File, format: &Format) -> io::Result<Self> {
        Ok(Self::new(file, format, |header| *header = Header::new(*format))?.0)
    }

    pub fn open(file: File, format: &Format) -> io::Result<(Self, FileHeader)> {
        Self::new(file, format, |_| {})
    }

    pub fn pager(&self) -> Pager {
        Pager::new(self.data.clone(), self.writable.clone())
    }

    pub fn allocator_state(&self) -> MutexGuard<'_, AllocatorState> {
        self.allocator_state.lock().unwrap()
    }

    pub fn snapshot(&mut self, root: FileHeader) -> io::Result<()> {
        let allocator_state = self.allocator_state.get_mut().unwrap();
        allocator_state.swap();
        self.version += 1;
        let pager = Pager::new(self.data.clone(), self.writable.clone());
        let page = unsafe { pager.page_mut(NULL_PAGE_NR) };
        let header_page = cast_mut::<Page, HeaderPage>(page);
        header_page.header.snapshot(State::new(
            self.version,
            *allocator_state,
            root.root,
            root.len,
        ));
        self.writable = MappedBitset::new(self.writable.len())?;
        self.synchronizer.sync();
        Ok(())
    }

    pub fn close(&self) {
        self.closing.store(true, Ordering::Relaxed);
    }

    pub fn is_closing(&self) -> bool {
        self.closing.load(Ordering::Relaxed)
    }
}
