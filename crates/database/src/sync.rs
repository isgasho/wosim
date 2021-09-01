use std::{
    sync::{
        atomic::{AtomicIsize, Ordering},
        Arc, Barrier,
    },
    thread::{Builder, JoinHandle},
};

use tracing::error;

use crate::mmap::MappedFile;

pub struct Synchronizer {
    state: Arc<State>,
    handle: Option<JoinHandle<()>>,
}

impl Synchronizer {
    pub fn new(data: MappedFile) -> Self {
        let state = Arc::new(State {
            barrier: Barrier::new(2),
            pending: AtomicIsize::new(0),
        });
        let handle = Some(Self::spawn(state.clone(), data));
        Self { state, handle }
    }

    pub fn sync(&self) {
        self.state.pending.fetch_add(1, Ordering::SeqCst);
        self.state.barrier.wait();
    }

    fn spawn(state: Arc<State>, data: MappedFile) -> JoinHandle<()> {
        Builder::new()
            .name("database synchronization thread".into())
            .spawn(move || loop {
                state.barrier.wait();
                if state.pending.fetch_sub(1, Ordering::SeqCst) == 0 {
                    return;
                }
                match data.sync() {
                    Ok(_) => {}
                    Err(error) => {
                        error!("{}", error);
                    }
                }
            })
            .unwrap()
    }
}

impl Drop for Synchronizer {
    fn drop(&mut self) {
        self.state.barrier.wait();
        self.handle.take().unwrap().join().unwrap();
    }
}

struct State {
    barrier: Barrier,
    pending: AtomicIsize,
}
