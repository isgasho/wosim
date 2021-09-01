use thiserror::Error;
use tokio::sync::mpsc::Sender;

use crate::{CancelError, ControlBarrier, Notification, Template};

pub struct Service {}
