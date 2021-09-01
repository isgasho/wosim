use std::{
    convert::TryInto,
    io::{self, ErrorKind},
};

use bytemuck::{bytes_of, Pod, Zeroable};
use sha3::{Digest, Sha3_512};

use crate::{
    allocator::AllocatorState,
    page::{Page, PageNr},
};

pub type Format = [u8; 256];

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Header {
    format: Format,
    snapshots: [Snapshot; 2],
}

impl Header {
    pub fn new(format: Format) -> Self {
        Self {
            format,
            snapshots: [
                Snapshot::new(State::default()),
                Snapshot::new(State::default()),
            ],
        }
    }

    pub fn snapshot(&mut self, state: State) {
        self.snapshots[(state.version % 2) as usize] = Snapshot::new(state);
    }

    pub fn validate(&self, format: &Format) -> io::Result<State> {
        if self.format != *format {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "database format mismatch",
            ));
        }
        self.last_snapshot()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Corrupted database"))
            .map(|s| s.state)
    }

    fn valid_snapshot(&self, index: usize) -> Option<&Snapshot> {
        let snapshot = &self.snapshots[index];
        if snapshot.state.version as usize % 2 == index % 2 && snapshot.is_valid() {
            Some(snapshot)
        } else {
            None
        }
    }

    fn last_snapshot(&self) -> Option<&Snapshot> {
        let s0 = self.valid_snapshot(0);
        let s1 = self.valid_snapshot(1);
        if s0.map(|s| s.state.version) <= s1.map(|s| s.state.version) {
            s1
        } else {
            s0
        }
    }
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct HeaderPage {
    pub header: Header,
    _padding9: [u8; 16],
    _padding0: [u8; 32],
    _padding1: [u8; 16],
    _padding2: [u8; 32],
    _padding3: [u8; 64],
    _padding4: [u8; 128],
    _padding5: [u8; 256],
    _padding6: [u8; 1024],
    _padding7: [u8; 2048],
    _padding8: [u8; 4096],
}

assert_eq_size!(HeaderPage, Page);

#[derive(Clone, Copy, Default, Pod, Zeroable)]
#[repr(C)]
pub struct State {
    pub version: u64,
    pub allocator: AllocatorState,
    pub root_nr: PageNr,
    pub root_len: u64,
}

impl State {
    pub fn new(version: u64, allocator: AllocatorState, root_nr: PageNr, root_len: u64) -> Self {
        Self {
            version,
            allocator,
            root_nr,
            root_len,
        }
    }

    pub fn checksum(&self) -> Checksum {
        let mut hasher = Sha3_512::new();
        hasher.update(self);
        let hash = hasher.finalize();
        hash[..].try_into().unwrap()
    }
}

impl AsRef<[u8]> for State {
    fn as_ref(&self) -> &[u8] {
        bytes_of(self)
    }
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Snapshot {
    state: State,
    checksum: Checksum,
}

impl Snapshot {
    fn new(state: State) -> Self {
        let checksum = state.checksum();
        Self { state, checksum }
    }

    fn is_valid(&self) -> bool {
        let checksum = self.state.checksum();
        checksum == self.checksum
    }
}

pub type Checksum = [u8; 64];
