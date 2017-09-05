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

//! Opaque, infinite serial.
//!
//! This module provides an opaque struct, [`Serial`](struct.Serial.html),
//! that can be incremented indefinitely. The only limitation is, you can only
//! compare values with a maximum difference of 2^32 increments between them.
//!
//! Due to the limits of the implementation, it's not possible to access
//! the actual value of a [`Serial`](struct.Serial.html), but you can compare
//! multiple instances of it to get the greatest or check if they're the same
//! one.

use std::cmp::Ordering;
use std::fmt;


/// Opaque, infinite serial.
///
/// Check out the [module documentation](index.html) for more details.
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Serial {
    increment: u32,
    alternate: bool,
}

impl Serial {
    /// Create a new Serial object, starting from zero.
    pub fn zero() -> Self {
        Serial {
            increment: 0,
            alternate: false,
        }
    }

    /// Return the Serial object following this one, without incrementing the
    /// current object in place.
    ///
    /// ```
    /// # use fisher::common::serial::Serial;
    /// let serial = Serial::zero();
    /// assert!(serial.next() > serial);
    /// ```
    pub fn next(&self) -> Serial {
        let mut serial = self.clone();

        let (new, overflowed) = serial.increment.overflowing_add(1);

        serial.increment = new;
        if overflowed {
            serial.alternate = !serial.alternate;
        }

        serial
    }

    /// Increment the current instance of Serial by one, and return the
    /// incremented value.
    ///
    /// ```
    /// # use fisher::common::serial::Serial;
    /// let mut serial = Serial::zero();
    /// let old = serial.clone();
    /// assert!(serial.incr() > old);
    /// ```
    pub fn incr(&mut self) -> Serial {
        let next = self.next();
        *self = next;
        *self
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
        } else {
            cmp
        }
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
        let old = Serial::zero();
        let new = old.next();

        assert!(old < new);
    }


    #[test]
    fn test_overflowing() {
        let mut old1 = Serial::zero();
        old1.increment = ::std::u32::MAX - 1;

        let old2 = old1.next();
        let old3 = old2.next();
        let new = old3.next();

        assert_eq!(old1.increment, ::std::u32::MAX - 1);
        assert_eq!(old2.increment, ::std::u32::MAX);
        assert_eq!(old3.increment, 0);
        assert_eq!(new.increment, 1);

        assert!(old1 < old2);
        assert!(old2 < old3);
        assert!(old3 < new);
        assert!(old1 < new);
    }


    #[test]
    fn test_incr() {
        let mut serial = Serial::zero();

        let original = serial.clone();
        serial.incr();

        assert!(serial > original);
    }
}
