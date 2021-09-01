use std::mem::size_of;

use bytemuck::{cast_mut, cast_ref};

use crate::{
    lock::Lock,
    page::{Page, PageNr, NULL_PAGE_NR, PAGE_SIZE},
};

type IndirectPage = [PageNr; FAN_OUT as usize];

const FAN_OUT: usize = PAGE_SIZE / size_of::<PageNr>();

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PageLevel {
    L0,
    L1,
    L2,
}

#[derive(Clone, Copy)]
struct PageIndex(usize, PageLevel);

impl PageIndex {
    pub fn new(index: usize, pages: usize) -> Self {
        assert!(index < pages);
        Self(index, PageLevel::from_pages(pages).unwrap())
    }

    pub const fn index(self) -> usize {
        self.0 / self.1.child_pages()
    }

    pub fn child(self) -> Self {
        Self(self.0 % self.1.child_pages(), self.1.child().unwrap())
    }

    pub const fn is_indirect(self) -> bool {
        self.1.is_indirect()
    }
}

#[derive(Clone, Copy)]
pub enum PageLookup {
    Immutable(usize, PageNr),
    Mutable(usize, PageNr),
    Invalid,
}

impl PageLookup {
    pub fn get<'a>(
        &mut self,
        root_nr: PageNr,
        pages: usize,
        index: usize,
        lock: &'a Lock,
    ) -> &'a Page {
        let page_nr = match *self {
            Self::Immutable(key, page_nr) => {
                if key == index {
                    page_nr
                } else {
                    let page_nr = find_page(root_nr, PageIndex::new(index, pages), lock);
                    *self = PageLookup::Immutable(index, page_nr);
                    page_nr
                }
            }
            Self::Mutable(key, page_nr) => {
                if key == index {
                    page_nr
                } else {
                    let page_nr = find_page(root_nr, PageIndex::new(index, pages), lock);
                    *self = PageLookup::Immutable(index, page_nr);
                    page_nr
                }
            }
            Self::Invalid => {
                let page_nr = find_page(root_nr, PageIndex::new(index, pages), lock);
                *self = PageLookup::Immutable(index, page_nr);
                page_nr
            }
        };
        unsafe { lock.page(page_nr) }
    }

    pub unsafe fn get_mut<'a>(
        &mut self,
        root_nr: &mut PageNr,
        pages: usize,
        index: usize,
        lock: &'a Lock,
    ) -> *mut Page {
        let page_nr = match *self {
            PageLookup::Immutable(key, page_nr) => {
                if key == index && lock.try_page_mut(page_nr).is_some() {
                    *self = PageLookup::Mutable(index, page_nr);
                    page_nr
                } else {
                    let page_nr = find_page_mut(root_nr, PageIndex::new(index, pages), lock);
                    *self = PageLookup::Mutable(index, page_nr);
                    page_nr
                }
            }
            PageLookup::Mutable(key, page_nr) => {
                if key == index {
                    page_nr
                } else {
                    let page_nr = find_page_mut(root_nr, PageIndex::new(index, pages), lock);
                    *self = PageLookup::Mutable(index, page_nr);
                    page_nr
                }
            }
            PageLookup::Invalid => {
                let page_nr = find_page_mut(root_nr, PageIndex::new(index, pages), lock);
                *self = PageLookup::Mutable(index, page_nr);
                page_nr
            }
        };
        lock.try_page_mut(page_nr).unwrap()
    }
}

fn find_page(page_nr: PageNr, index: PageIndex, lock: &Lock) -> PageNr {
    if index.is_indirect() {
        find_page(
            cast_ref::<Page, IndirectPage>(unsafe { lock.page(page_nr) })[index.index()],
            index.child(),
            lock,
        )
    } else {
        page_nr
    }
}

fn find_page_mut(page_nr: &mut PageNr, index: PageIndex, lock: &Lock) -> PageNr {
    let page = unsafe { lock.page_mut(page_nr) };
    if index.is_indirect() {
        find_page_mut(
            &mut cast_mut::<Page, IndirectPage>(page)[index.index()],
            index.child(),
            lock,
        )
    } else {
        *page_nr
    }
}

impl PageLevel {
    pub const fn is_indirect(self) -> bool {
        match self {
            Self::L0 => false,
            Self::L1 => true,
            Self::L2 => true,
        }
    }

    pub fn from_pages(pages: usize) -> Option<Self> {
        match pages {
            0 => None,
            1 => Some(Self::L0),
            2..=2048 => Some(Self::L1),
            2049..=4194304 => Some(Self::L2),
            _ => panic!(),
        }
    }

    pub const fn child(self) -> Option<Self> {
        match self {
            Self::L0 => None,
            Self::L1 => Some(Self::L0),
            Self::L2 => Some(Self::L1),
        }
    }

    pub fn parent(this: Option<Self>) -> Self {
        match this {
            None => Self::L0,
            Some(Self::L0) => Self::L1,
            Some(Self::L1) => Self::L2,
            Some(Self::L2) => panic!(),
        }
    }

    pub const fn child_pages(self) -> usize {
        match self {
            Self::L0 => 0,
            Self::L1 => 1,
            Self::L2 => 2048,
        }
    }
}

pub fn reallocate(root_nr: &mut PageNr, mut current_pages: usize, new_pages: usize, lock: &Lock) {
    let mut current_root_level = PageLevel::from_pages(current_pages);
    let new_root_level = PageLevel::from_pages(new_pages);
    while current_root_level < new_root_level {
        increment_levels(root_nr, &mut current_pages, &mut current_root_level, lock);
    }
    while current_root_level > new_root_level {
        decrement_levels(root_nr, &mut current_pages, &mut current_root_level, lock);
    }
    match current_pages.cmp(&new_pages) {
        std::cmp::Ordering::Less => {
            allocate(
                root_nr,
                current_pages,
                new_pages,
                current_root_level.unwrap(),
                lock,
            );
        }
        std::cmp::Ordering::Equal => {}
        std::cmp::Ordering::Greater => {
            deallocate_from(root_nr, new_pages, current_root_level.unwrap(), lock)
        }
    }
}

fn increment_levels(
    root_nr: &mut PageNr,
    pages: &mut usize,
    current_root_level: &mut Option<PageLevel>,
    lock: &Lock,
) {
    let mut new_root_nr = 0;
    let page = unsafe { lock.page_mut(&mut new_root_nr) };
    let next_root_level = PageLevel::parent(*current_root_level);
    if let Some(level) = *current_root_level {
        allocate(root_nr, *pages, next_root_level.child_pages(), level, lock);
    }
    cast_mut::<Page, IndirectPage>(page)[0] = *root_nr;
    *root_nr = new_root_nr;
    *pages = next_root_level.child_pages();
    *current_root_level = Some(next_root_level);
}

fn decrement_levels(
    root_nr: &mut PageNr,
    pages: &mut usize,
    current_root_level: &mut Option<PageLevel>,
    lock: &Lock,
) {
    let root_level = (*current_root_level).unwrap();
    let new_root_nr = if root_level.is_indirect() {
        let page = unsafe { lock.page(*root_nr) };
        cast_ref::<Page, IndirectPage>(page)[0]
    } else {
        NULL_PAGE_NR
    };
    deallocate_full(*root_nr, 1, root_level, lock);
    *root_nr = new_root_nr;
    *pages = root_level.child_pages();
    *current_root_level = root_level.child();
}

fn allocate(page_nr: &mut PageNr, from: usize, to: usize, level: PageLevel, lock: &Lock) {
    let page = unsafe { lock.page_mut(page_nr) };
    if from == to {
        return;
    }
    if level.is_indirect() {
        let page = cast_mut::<Page, IndirectPage>(page);
        let from_index = from / level.child_pages();
        let to_index = to / level.child_pages();
        let child_level = level.child().unwrap();
        if from_index == to_index {
            allocate(
                &mut page[from_index],
                from % level.child_pages(),
                to % level.child_pages(),
                child_level,
                lock,
            );
        } else {
            allocate(
                &mut page[from_index],
                from % level.child_pages(),
                level.child_pages(),
                child_level,
                lock,
            );
            for child in page[from_index + 1..to_index].iter_mut() {
                allocate_full(child, child_level, lock)
            }
            if to % level.child_pages() != 0 {
                allocate(
                    &mut page[to_index],
                    0,
                    to % level.child_pages(),
                    child_level,
                    lock,
                );
            }
        }
    }
}

fn allocate_full(page_nr: &mut PageNr, level: PageLevel, lock: &Lock) {
    let page = unsafe { lock.page_mut(page_nr) };
    if level.is_indirect() {
        let child_level = level.child().unwrap();
        for page_nr in cast_mut::<Page, IndirectPage>(page).iter_mut() {
            allocate_full(page_nr, child_level, lock);
        }
    }
}

fn deallocate_from(page_nr: &mut PageNr, from: usize, level: PageLevel, lock: &Lock) {
    if level.is_indirect() {
        let mut index = from / level.child_pages();
        let child_from = from % level.child_pages();
        if index == 0 && child_from == 0 {
            deallocate_full(*page_nr, 0, level, lock);
            *page_nr = NULL_PAGE_NR;
            return;
        }
        let child_level = level.child().unwrap();
        let page = cast_mut::<Page, IndirectPage>(unsafe { lock.page_mut(page_nr) });
        deallocate_from(&mut page[index], child_from, child_level, lock);
        index += 1;
        while index < FAN_OUT && page[index] != NULL_PAGE_NR {
            deallocate_full(page[index], 0, child_level, lock);
            page[index] = NULL_PAGE_NR;
            index += 1;
        }
    } else {
        unsafe { lock.deallocate(*page_nr) };
        *page_nr = NULL_PAGE_NR;
    }
}

fn deallocate_full(page_nr: PageNr, mut index: usize, level: PageLevel, lock: &Lock) {
    let page = unsafe { lock.page(page_nr) };
    if level.is_indirect() {
        let page = cast_ref::<Page, IndirectPage>(page);
        while index < FAN_OUT && page[index] != NULL_PAGE_NR {
            deallocate_full(page[index], 0, level.child().unwrap(), lock);
            index += 1;
        }
    }
    unsafe { lock.deallocate(page_nr) }
}
