pub trait MaxOkFilterMap<T, U: Ord, E, F: Fn(T) -> Result<Option<U>, E>> {
    fn max_ok_filter_map(self, f: F) -> Result<Option<U>, E>;
}

impl<T, U: Ord, E, F: Fn(T) -> Result<Option<U>, E>, I: Iterator<Item = T>>
    MaxOkFilterMap<T, U, E, F> for I
{
    fn max_ok_filter_map(mut self, f: F) -> Result<Option<U>, E> {
        self.try_fold(None, |acc: Option<U>, x| match acc {
            Some(v) => match f(x) {
                Ok(x) => match x {
                    Some(x) => Ok(Some(v.max(x))),
                    None => Ok(Some(v)),
                },
                Err(e) => Err(e),
            },
            None => f(x),
        })
    }
}
