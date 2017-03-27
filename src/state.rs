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

use std::sync::atomic::{AtomicUsize, Ordering};


#[derive(Debug)]
pub struct State {
    next_hook_id: AtomicUsize,
}

impl State {

    pub fn new() -> Self {
        State {
            next_hook_id: AtomicUsize::new(0),
        }
    }

    pub fn next_hook_id(&self) -> usize {
        self.next_hook_id.fetch_add(1, Ordering::SeqCst)
    }
}


#[cfg(test)]
mod tests {
    use super::State;


    #[test]
    fn test_next_hook_id() {
        // State must always increment
        let state1 = State::new();
        assert_eq!(state1.next_hook_id(), 0);
        assert_eq!(state1.next_hook_id(), 1);
        assert_eq!(state1.next_hook_id(), 2);

        // Test on a different state instance
        let state2 = State::new();
        assert_eq!(state2.next_hook_id(), 0);
        assert_eq!(state2.next_hook_id(), 1);
        assert_eq!(state2.next_hook_id(), 2);
    }
}
