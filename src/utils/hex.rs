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

use errors::{FisherResult, ErrorKind};


pub fn from_hex(input: &str) -> FisherResult<Vec<u8>> {
    let mut result = Vec::with_capacity(input.len() / 2);

    let mut pending: u8 = 0;
    let mut buffer: u8 = 0;
    let mut current: u8;
    for (i, byte) in input.bytes().enumerate() {
        pending += 1;

        current = match byte {
            b'0'...b'9' => byte - b'0',
            b'a'...b'f' => byte - b'a' + 10,
            b'A'...b'F' => byte - b'A' + 10,
            _ => {
                return Err(ErrorKind::InvalidHexChar(
                    input[i..].chars().next().unwrap()
                ).into());
            }
        };

        if pending == 1 {
            buffer = current;
        } else {
            result.push(buffer * 16 + current);
            pending = 0;
        }
    }

    if pending != 0 {
        Err(ErrorKind::InvalidHexLength.into())
    } else {
        Ok(result)
    }
}


#[cfg(test)]
mod tests {
    use errors::ErrorKind;

    use super::from_hex;

    #[test]
    fn test_from_hex() {
        assert_eq!(from_hex("68656c6c6f").unwrap(), b"hello");
        assert_eq!(from_hex("68656C6C6F").unwrap(), b"hello");
        assert_err!(from_hex("0"), ErrorKind::InvalidHexLength);
        assert_err!(from_hex("fg"), ErrorKind::InvalidHexChar('g'));
    }
}
