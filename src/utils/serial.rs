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

use std::cmp::Ordering;
use std::fmt;


#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Serial {
    increment: u32,
    alternate: bool,
}

impl Serial {

    pub fn zero() -> Self {
        Serial {
            increment: 0,
            alternate: false,
        }
    }

    pub fn next(&mut self) {
        let (new, overflowed) = self.increment.overflowing_add(1);

        self.increment = new;
        if overflowed {
            self.alternate = ! self.alternate;
        }
    }
}

impl fmt::Debug for Serial {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Serial")
    }
}

impl Ord for Serial {

    fn cmp(&self, other: &Serial) -> Ordering {
        let cmp = self.increment.cmp(&other.increment);

        if self.alternate != other.alternate {
            cmp.reverse()
        } else { cmp }
    }
}

impl PartialOrd for Serial {

    fn partial_cmp(&self, other: &Serial) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}


#[cfg(test)]
mod tests {
    use super::Serial;


    #[test]
    fn test_basic_ordering() {
        let mut serial = Serial::zero();

        let old = serial.clone();
        serial.next();

        assert!(old < serial);
    }


    #[test]
    fn test_overflowing() {
        let mut serial = Serial::zero();
        serial.increment = ::std::u32::MAX - 1;

        let old1 = serial.clone();
        serial.next();

        let old2 = serial.clone();
        serial.next();

        let old3 = serial.clone();
        serial.next();

        assert_eq!(old1.increment, ::std::u32::MAX - 1);
        assert_eq!(old2.increment, ::std::u32::MAX);
        assert_eq!(old3.increment, 0);
        assert_eq!(serial.increment, 1);

        assert!(old1 < old2);
        assert!(old2 < old3);
        assert!(old3 < serial);
        assert!(old1 < serial);
    }
}
