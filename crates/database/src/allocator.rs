use std::{mem::swap, sync::MutexGuard};

use bytemuck::{Pod, Zeroable};

use crate::{
    free_list::FreeList,
    page::{PageNr, Pager},
};

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct AllocatorState {
    previous_free: FreeList,
    current_free: FreeList,
    last_page: u32,
}

impl AllocatorState {
    pub fn swap(&mut self) {
        swap(&mut self.previous_free, &mut self.current_free);
        self.current_free.reset_front();
    }
}

impl Default for AllocatorState {
    fn default() -> Self {
        Self {
            previous_free: FreeList::default(),
            current_free: FreeList::default(),
            last_page: 1,
        }
    }
}

pub struct Allocator<'a> {
    state: MutexGuard<'a, AllocatorState>,
    pager: &'a Pager,
    append: Vec<u32>,
    prepend: Vec<u32>,
}

impl<'a> Allocator<'a> {
    pub fn new(state: MutexGuard<'a, AllocatorState>, pager: &'a Pager) -> Self {
        Self {
            state,
            pager,
            append: Vec::new(),
            prepend: Vec::new(),
        }
    }

    pub fn allocate(&mut self) -> PageNr {
        let nr = if let Some(nr) = self.append.pop() {
            nr
        } else if let Some(nr) = unsafe { self.state.current_free.pop_back(self.pager) } {
            nr
        } else if let Some(nr) = unsafe { self.state.previous_free.pop_back(self.pager) } {
            nr
        } else {
            self.state.last_page += 1;
            self.state.last_page
        };
        unsafe { self.pager.enable_write(nr) };
        nr
    }

    pub unsafe fn deallocate(&mut self, nr: PageNr) {
        if self.pager.can_write(nr) {
            self.append.push(nr);
        } else {
            self.prepend.push(nr);
            if let Some(old_nr) = self.state.current_free.shift_front(self.pager) {
                self.append.push(old_nr);
            }
        }
    }

    pub unsafe fn reallocate(&mut self, nr: PageNr) -> PageNr {
        self.prepend.push(nr);
        if let Some(new_nr) = self.state.current_free.shift_front(self.pager) {
            self.pager.enable_write(new_nr);
            new_nr
        } else {
            self.allocate()
        }
    }

    pub fn pager(&self) -> &'a Pager {
        self.pager
    }

    pub fn current_free(&mut self) -> &mut FreeList {
        &mut self.state.current_free
    }

    unsafe fn resolve(&mut self) {
        while !(self.prepend.is_empty() && self.append.is_empty()) {
            let mut prepend = Vec::new();
            swap(&mut prepend, &mut self.prepend);
            FreeList::prepend(self, prepend);
            let mut append = Vec::new();
            swap(&mut append, &mut self.append);
            FreeList::append(self, append);
        }
    }
}

impl<'a> Drop for Allocator<'a> {
    fn drop(&mut self) {
        unsafe { self.resolve() }
    }
}
