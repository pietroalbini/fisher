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

use std::collections::HashMap;

use errors::{FisherResult, ErrorKind};


pub fn parse_env(content: &str) -> FisherResult<HashMap<&str, &str>> {
    let mut result = HashMap::new();

    for line in content.split("\n") {
        // Skip empty lines
        if line.trim() == "" {
            continue;
        }

        let parts: Vec<&str> = line.splitn(2, "=").take(2).collect();
        if parts.len() != 2 {
            return Err(ErrorKind::InvalidInput(
                format!("Invalid env received: {}", line)
            ).into());
        }

        let key = parts.get(0).unwrap();
        let value = parts.get(1).unwrap();
        result.insert(*key, *value);
    }

    Ok(result)
}


#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use errors::ErrorKind;

    use super::parse_env;


    #[test]
    fn test_parse_env() {
        // Test with an empty env
        assert_eq!(parse_env("").unwrap(), HashMap::new());

        // Test with multiple elements
        let mut expected = HashMap::new();
        expected.insert("A", "b");
        expected.insert("B", "c=d=e");
        assert_eq!(parse_env("A=b\n\nB=c=d=e\n").unwrap(), expected);

        // Test with invalid data
        let res = parse_env("A=b\nB");
        assert!(res.is_err());
        match *res.err().unwrap().kind() {
            ErrorKind::InvalidInput(..) => {
                assert!(true)
            },
            _ => {
                panic!("Wrong error received!");
            },
        };
    }
}
