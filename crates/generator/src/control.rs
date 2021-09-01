use std::{
    future::Future,
    mem::swap,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Poll, Waker},
};

use thiserror::Error;

#[derive(Clone)]
pub struct Control(Arc<Mutex<ControlState>>);

impl Control {
    pub fn new() -> (Self, ControlBarrier) {
        let state = Arc::new(Mutex::new(ControlState::Running));
        (Self(state.clone()), ControlBarrier(state))
    }

    pub fn pause(&self) {
        let mut state = self.0.lock().unwrap();
        if let ControlState::Running = *state {
            *state = ControlState::PausePending;
        }
    }

    pub fn unpause(&self) {
        let mut state = self.0.lock().unwrap();
        if let ControlState::Cancelled = *state {
            return;
        }
        let mut old_state = ControlState::Running;
        swap(&mut *state, &mut old_state);
        if let ControlState::Paused(waker) = old_state {
            waker.wake();
        }
    }

    pub fn cancel(&self) {
        let mut state = self.0.lock().unwrap();
        let mut old_state = ControlState::Cancelled;
        swap(&mut *state, &mut old_state);
        if let ControlState::Paused(waker) = old_state {
            waker.wake();
        }
    }
}

pub struct ControlBarrier(Arc<Mutex<ControlState>>);

impl ControlBarrier {
    pub fn wait(&mut self) -> ControlWait {
        ControlWait(&self.0)
    }
}

pub struct ControlWait<'a>(&'a Mutex<ControlState>);

impl<'a> Future for ControlWait<'a> {
    type Output = Result<(), CancelError>;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let mut state = self.0.lock().unwrap();
        match *state {
            ControlState::Running => Poll::Ready(Ok(())),
            ControlState::PausePending => {
                *state = ControlState::Paused(cx.waker().clone());
                Poll::Pending
            }
            ControlState::Paused(_) => panic!("invalid state"),
            ControlState::Cancelled => Poll::Ready(Err(CancelError)),
        }
    }
}

#[derive(Debug, Error)]
#[error("control cancelled")]
pub struct CancelError;

enum ControlState {
    Running,
    PausePending,
    Paused(Waker),
    Cancelled,
}
