pub struct Cache<K, V>(Option<(K, V)>);

impl<K: Eq, V> Cache<K, V> {
    pub fn try_get<E>(&mut self, key: K, f: impl FnOnce() -> Result<V, E>) -> Result<&mut V, E> {
        if let Some(pair) = &mut self.0 {
            if pair.0 != key {
                pair.0 = key;
                pair.1 = f()?;
            }
        } else {
            self.0 = Some((key, f()?))
        }
        Ok(&mut self.0.as_mut().unwrap().1)
    }

    pub fn get(&mut self, key: K, f: impl FnOnce() -> V) -> &mut V {
        if let Some(pair) = &mut self.0 {
            if pair.0 != key {
                pair.0 = key;
                pair.1 = f();
            }
        } else {
            self.0 = Some((key, f()))
        }
        &mut self.0.as_mut().unwrap().1
    }

    pub fn unwrap(&mut self) -> &mut V {
        &mut self.0.as_mut().unwrap().1
    }
}

impl<K, V> Default for Cache<K, V> {
    fn default() -> Self {
        Self(None)
    }
}
