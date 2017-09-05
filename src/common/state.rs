// Copyright (C) 2017 Pietro Albini
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

//! Fisher's global state.
//!
//! This module contains the code that keeps the Fisher global state. This is
//! used for example to generate unique IDs across the codebase. The main
//! [`State`](struct.State.html) struct is also marked as Sync and Send, so
//! it can be used across threads without locking.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::cmp::PartialOrd;
use std::cmp::Ordering as CmpOrdering;


/// This enum represents a kind of ID.
///
/// You should use this to specify which ID you do want.

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum IdKind {
    /// This kind should be used to identify hooks.
    HookId,

    /// This kind should be used to identify threads.
    ThreadId,

    #[doc(hidden)] __NonExaustiveMatch,
}


/// This struct contains an unique ID.
///
/// The struct is intentionally opaque, so you won't be able to get the actual
/// value of the ID, but you can compare multiple IDs to get which one is
/// greater, and check if multiple IDs are equal. This is done to be able to
/// swap the inner implementation without breaking any code.

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct UniqueId {
    id: usize,
    kind: IdKind,
}

impl PartialOrd for UniqueId {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        if self.kind == other.kind {
            self.id.partial_cmp(&other.id)
        } else {
            None
        }
    }
}


/// This struct keeps the global state of Fisher.

#[derive(Debug)]
pub struct State {
    counter: AtomicUsize,
}

impl State {
    /// Create a new instance of the struct.
    pub fn new() -> Self {
        State {
            counter: AtomicUsize::new(0),
        }
    }

    /// Get the next ID for a specific [`IdKind`](enum.IdKind.html). The ID is
    /// guaranteed to be unique and greater than the last ID.
    pub fn next_id(&self, kind: IdKind) -> UniqueId {
        UniqueId {
            id: self.counter.fetch_add(1, Ordering::SeqCst),
            kind: kind,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::{IdKind, State};


    #[test]
    fn test_next_id() {
        // State must always increment
        let state = State::new();
        let id1 = state.next_id(IdKind::HookId);
        let id2 = state.next_id(IdKind::HookId);
        let id3 = state.next_id(IdKind::ThreadId);

        assert!(id1 < id2);
        assert!(id1 == id1);
        assert!(id1 != id2);
        assert!(id1 != id3);
    }
}
