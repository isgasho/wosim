#[derive(Clone, Copy)]
#[must_use]
pub enum HandleFlow {
    Unhandled,
    Handled,
}

impl HandleFlow {
    pub fn is_handled(self) -> bool {
        match self {
            HandleFlow::Unhandled => false,
            HandleFlow::Handled => true,
        }
    }
}
