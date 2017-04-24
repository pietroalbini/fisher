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

use fisher_common::errors::{Result, ErrorKind};


pub fn parse_env(line: &str) -> Result<(&str, &str)> {
    if let Some(pos) = line.find('=') {
        let (key, value) = line.split_at(pos);
        Ok((key, &value[1..]))
    } else {
        Err(ErrorKind::InvalidInput(
            format!("Not a valid environment definition: {}", line)
        ).into())
    }
}


#[cfg(test)]
mod tests {
    use super::parse_env;


    #[test]
    fn test_parse_env() {
        assert!(parse_env("b").is_err());
        assert_eq!(parse_env("a=b").unwrap(), ("a", "b"));
        assert_eq!(parse_env("a=b=c").unwrap(), ("a", "b=c"));
    }
}
