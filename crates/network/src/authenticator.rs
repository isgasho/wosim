use std::sync::Arc;

use crate::{RawConnection, RawMessageSender};

pub trait Authenticator: 'static + Send + Sync {
    fn authenticate(
        &self,
        token: &str,
        connection: Arc<RawConnection>,
    ) -> Result<RawMessageSender, String>;
}
