use std::ops::DerefMut;

use bytemuck::{cast_slice, cast_slice_mut, Pod, TransparentWrapper};

use crate::{
    lock::Lock,
    page::{Page, PageNr, PAGE_SIZE},
};

use super::{branch::Branch, leaf::Leaf};

pub trait NodePage: DerefMut<Target = [u8; PAGE_SIZE]> + Clone + Pod {
    fn footer(&self) -> &u16 {
        &cast_slice(&self[PAGE_SIZE - 2..PAGE_SIZE])[0]
    }

    fn footer_mut(&mut self) -> &mut u16 {
        &mut cast_slice_mut(&mut self[PAGE_SIZE - 2..PAGE_SIZE])[0]
    }

    fn len(&self) -> usize {
        (*self.footer() & 0x7fff) as usize
    }

    fn set_len(&mut self, len: usize) {
        *self.footer_mut() &= 0x8000;
        *self.footer_mut() |= len as u16;
    }

    fn is_leaf(&self) -> bool {
        *self.footer() & 0x8000 == 0
    }
}

impl NodePage for Page {}

pub unsafe fn allocate<'a>(lock: &'a Lock, is_leaf: bool) -> (PageNr, &'a mut Page) {
    let page_nr = lock.allocate();
    let page = lock.try_page_mut(page_nr).unwrap();
    *page.footer_mut() = if is_leaf { 0 } else { 0x8000 };
    (page_nr, page)
}

pub fn node<K: Pod + Ord, V: Pod>(page: &Page) -> NodeRef<K, V> {
    if page.is_leaf() {
        NodeRef::Leaf(Leaf::wrap_ref(page))
    } else {
        NodeRef::Branch(Branch::wrap_ref(page))
    }
}

pub enum NodeRef<'a, K: Pod + Ord, V: Pod> {
    Branch(&'a Branch<K>),
    Leaf(&'a Leaf<K, V>),
}
