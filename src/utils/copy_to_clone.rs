// Copyright (C) 2016 Pietro Albini
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

use std::ops::Deref;


/// This struct wraps a type implementing `Copy` in order to `Clone` it.
///
/// It was created because you can currently clone function pointers without
/// references in the arguments, but you can't clone them if they accepts
/// references as arguments. This bug is tracked as [Rust issue #28229]
/// (https://github.com/rust-lang/rust/issues/28229).
///
/// # Examples
///
/// ```
/// fn print(string: &str) {
///     println!("{}", string);
/// }
///
/// fn main() {
///     // Create a pointer
///     let pointer: CopyToClone<fn(&str)> = CopyToClone::new(print);
///
///     // Clone the pointer
///     let cloned = pointer.clone();
///
///     // Dereference the function and call it
///     (*cloned)("Hello world");
/// }
/// ```

pub struct CopyToClone<Value: Copy> {
    value: Value,
}

impl<Value: Copy> CopyToClone<Value> {

    /// Create a new `CopyToClone` instance.
    ///
    /// The `value` must implement the `Copy` trait.

    pub fn new(value: Value) -> CopyToClone<Value> {
        CopyToClone {
            value: value,
        }
    }
}

impl<Value: Copy> Clone for CopyToClone<Value> {

    fn clone(&self) -> Self {
        Self::new(self.value)
    }
}

impl<Value: Copy> Deref for CopyToClone<Value> {
    type Target = Value;

    fn deref(&self) -> &Value {
        &self.value
    }
}


#[cfg(test)]
mod tests {
    use super::CopyToClone;

    #[test]
    fn test_copy_to_clone() {
        // Try to clone a pointer to a function without a ref as argument
        let wrapped = CopyToClone::new(func_without_ref);
        let cloned = wrapped.clone();
        assert_eq!((*cloned)("test".to_string()), true);

        // Try to clone a pointer to a function without a ref as argument
        let wrapped = CopyToClone::new(func_with_ref);
        let cloned = wrapped.clone();
        assert_eq!((*cloned)("test"), true);
    }

    fn func_with_ref(string: &str) -> bool {
        string == "test"
    }

    fn func_without_ref(string: String) -> bool {
        string == "test"
    }
}
