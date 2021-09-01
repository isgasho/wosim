use std::mem::size_of;

use bytemuck::{cast_mut, cast_ref, Pod, Zeroable};

use crate::{
    allocator::Allocator,
    page::{Page, PageNr, Pager, NULL_PAGE_NR, PAGE_SIZE},
};

const FAN_OUT: usize = PAGE_SIZE / size_of::<PageNr>();
const MAX_DEPTH: usize = 2;
const ENTRIES_PER_DEPTH: [usize; MAX_DEPTH + 1] = [FAN_OUT.pow(2), FAN_OUT, 1];

type FreeListPage = [PageNr; FAN_OUT];

#[derive(Clone, Copy, Default, Pod, Zeroable)]
#[repr(C)]
pub struct FreeList {
    root: PageNr,
    front: u32,
    back: u32,
}

impl FreeList {
    pub unsafe fn shift_front(&mut self, pager: &Pager) -> Option<PageNr> {
        if self.front < self.back {
            let nr = self.get(self.front, pager);
            self.front += 1;
            Some(nr)
        } else {
            self.front += 1;
            self.back += 1;
            None
        }
    }

    pub fn reset_front(&mut self) {
        self.front = 0;
    }

    pub unsafe fn pop_back(&mut self, pager: &Pager) -> Option<PageNr> {
        if self.front < self.back {
            self.back -= 1;
            Some(self.get(self.back, pager))
        } else {
            None
        }
    }

    unsafe fn set(allocator: &mut Allocator, index: u32, value: PageNr) {
        let index = index as usize;
        allocator.current_free().root = Self::set_inner(
            allocator.current_free().root,
            index,
            value,
            0,
            allocator,
            allocator.pager(),
        )
    }

    unsafe fn get(&self, index: u32, pager: &Pager) -> PageNr {
        let index = index as usize;
        Self::get_in_page(self.root, index, 0, pager)
    }

    unsafe fn get_in_page(page_nr: PageNr, index: usize, depth: usize, pager: &Pager) -> PageNr {
        assert_ne!(page_nr, NULL_PAGE_NR);
        let child_index = index / ENTRIES_PER_DEPTH[depth];
        let child = cast_ref::<Page, FreeListPage>(pager.page(page_nr))[child_index];
        if depth < MAX_DEPTH {
            Self::get_in_page(child, index % ENTRIES_PER_DEPTH[depth], depth + 1, pager)
        } else {
            child
        }
    }

    pub unsafe fn prepend(allocator: &mut Allocator, nrs: Vec<PageNr>) {
        let mut index = allocator.current_free().front - nrs.len() as u32;
        for nr in nrs {
            Self::set(allocator, index, nr);
            index += 1;
        }
    }

    pub unsafe fn append(allocator: &mut Allocator, nrs: Vec<PageNr>) {
        for nr in nrs {
            let index = allocator.current_free().back;
            Self::set(allocator, index, nr);
            allocator.current_free().back += 1;
        }
    }

    unsafe fn set_inner(
        page_nr: PageNr,
        index: usize,
        value: PageNr,
        depth: usize,
        allocator: &mut Allocator<'_>,
        pager: &Pager,
    ) -> PageNr {
        let (page_nr, page) = if page_nr == NULL_PAGE_NR {
            let page_nr = allocator.allocate();
            let page = pager.page_mut(page_nr);
            *page = Page::zeroed();
            (page_nr, page)
        } else if pager.can_write(page_nr) {
            (page_nr, pager.page_mut(page_nr))
        } else {
            let new_nr = allocator.reallocate(page_nr);
            let page = pager.copy_page_mut(page_nr, new_nr);
            (new_nr, page)
        };
        let child_index = index / ENTRIES_PER_DEPTH[depth];
        let child = &mut cast_mut::<Page, FreeListPage>(page)[child_index];
        if depth < MAX_DEPTH {
            *child = Self::set_inner(
                *child,
                index % ENTRIES_PER_DEPTH[depth],
                value,
                depth + 1,
                allocator,
                pager,
            );
        } else {
            *child = value;
        }
        page_nr
    }
}
