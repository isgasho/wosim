use std::ops::{Index, IndexMut};

pub const FRAMES_IN_FLIGHT: usize = 2;

pub struct PerFrame<T>([T; FRAMES_IN_FLIGHT]);

impl<T> PerFrame<T> {
    pub fn new<E>(mut f: impl FnMut(usize) -> Result<T, E>) -> Result<Self, E> {
        Ok(Self([f(0)?, f(1)?]))
    }
}

impl<T> Index<usize> for PerFrame<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index % FRAMES_IN_FLIGHT]
    }
}

impl<T> IndexMut<usize> for PerFrame<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index % FRAMES_IN_FLIGHT]
    }
}
