use std::{io, sync::Arc};

use atomic_refcell::AtomicRefCell;

use crate::{file::FileHeader, lock::Lock, raw::RawDatabase};

#[derive(Clone)]
pub struct DatabaseRef(Arc<AtomicRefCell<RawDatabase>>);

impl DatabaseRef {
    pub(crate) fn new(raw: RawDatabase) -> Self {
        Self(Arc::new(AtomicRefCell::new(raw)))
    }

    pub(crate) fn lock(&self) -> Lock<'_> {
        Lock::new(self.0.borrow())
    }

    pub(crate) fn snapshot(&self, root: FileHeader) -> io::Result<()> {
        self.0.borrow_mut().snapshot(root)
    }
}
