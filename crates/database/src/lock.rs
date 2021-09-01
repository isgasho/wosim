use crate::{
    allocator::Allocator,
    page::{Page, PageNr, Pager, NULL_PAGE_NR},
    raw::RawDatabase,
};
use atomic_refcell::AtomicRef;
use bytemuck::Zeroable;

pub struct Lock<'a> {
    pager: Pager,
    database: AtomicRef<'a, RawDatabase>,
}

impl<'a> Lock<'a> {
    pub fn new(database: AtomicRef<'a, RawDatabase>) -> Self {
        let pager = database.pager();
        Self { pager, database }
    }

    pub fn allocate(&self) -> PageNr {
        self.allocator().allocate()
    }

    pub fn close(&self) {
        self.database.close()
    }

    pub fn is_closing(&self) -> bool {
        self.database.is_closing()
    }

    pub unsafe fn deallocate(&self, nr: PageNr) {
        self.allocator().deallocate(nr)
    }

    pub unsafe fn reallocate(&self, nr: PageNr) -> PageNr {
        self.allocator().reallocate(nr)
    }

    pub unsafe fn page(&self, nr: PageNr) -> &Page {
        assert_ne!(nr, NULL_PAGE_NR);
        self.pager.page(nr)
    }

    pub unsafe fn page_mut(&self, nr: &mut PageNr) -> &mut Page {
        if *nr == NULL_PAGE_NR {
            *nr = self.allocate_zeroed();
            self.pager.page_mut(*nr)
        } else if self.pager.can_write(*nr) {
            self.pager.page_mut(*nr)
        } else {
            let new_nr = self.reallocate(*nr);
            let page = self.pager.copy_page_mut(*nr, new_nr);
            *nr = new_nr;
            page
        }
    }

    pub unsafe fn try_page_mut(&self, nr: PageNr) -> Option<&mut Page> {
        if self.pager.can_write(nr) {
            Some(self.pager.page_mut(nr))
        } else {
            None
        }
    }

    pub fn allocate_zeroed(&self) -> PageNr {
        let nr = self.allocate();
        unsafe { *self.pager.page_mut(nr) = Page::zeroed() };
        nr
    }

    fn allocator(&self) -> Allocator<'_> {
        Allocator::new(self.database.allocator_state(), &self.pager)
    }
}

impl<'a> Clone for Lock<'a> {
    fn clone(&self) -> Self {
        Self {
            database: AtomicRef::clone(&self.database),
            pager: self.database.pager(),
        }
    }
}
