pub struct OnceValue<T>(Option<T>);

impl<T> OnceValue<T> {
    pub fn get(&mut self, f: impl FnOnce() -> T) -> &mut T {
        if self.0.is_none() {
            self.0 = Some(f())
        }
        self.0.as_mut().unwrap()
    }
}

impl<T> Default for OnceValue<T> {
    fn default() -> Self {
        Self(None)
    }
}
