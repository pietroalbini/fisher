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

use std::fmt;
use std::result::Result as StdResult;
use std::str::FromStr;

use serde::de::{Error as DeError, Visitor, Deserialize, Deserializer};

use common::prelude::*;


/// Parse a time string and return the equivalent time in seconds.
///
/// Examples of time strings are "10" (for 10 seconds), "1d" (for 86400) or
/// 1m10s (for 70).
pub fn parse_time(input: &str) -> Result<usize> {
    let mut result = 0;
    let mut number_temp;
    let mut number_len = 0;

    for (i, c) in input.chars().enumerate() {
        match c {
            '0' ... '9' => number_len += 1,
            _ => {
                if number_len > 0 {
                    number_temp = input[i-number_len..i].parse::<usize>()?;

                    match c {
                        's' => {},
                        'm' => number_temp *= 60,
                        'h' => number_temp *= 60 * 60,
                        'd' => number_temp *= 60 * 60 * 24,
                        _ => return Err(
                            ErrorKind::InvalidTimeString(input.into()).into()
                        ),
                    }

                    number_len = 0;
                    result += number_temp;
                } else {
                    return Err(
                        ErrorKind::InvalidTimeString(input.into()).into()
                    )
                }
            },
        }
    }

    if number_len > 0 {
        result += input[input.len() - number_len..].parse::<usize>()?;
    }

    Ok(result)
}


#[derive(Debug, PartialEq, Eq)]
pub struct TimeString(u64);

impl TimeString {
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl From<u64> for TimeString {
    fn from(num: u64) -> Self {
        TimeString(num)
    }
}

impl FromStr for TimeString {
    type Err = Error;

    fn from_str(s: &str) -> Result<TimeString> {
        Ok(TimeString(parse_time(s)? as u64))
    }
}

struct TimeStringVisitor;

impl<'de> Visitor<'de> for TimeStringVisitor {
    type Value = TimeString;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a positive number or a time string")
    }

    fn visit_str<E: DeError>(self, s: &str) -> StdResult<TimeString, E> {
        match parse_time(s) {
            Ok(time) => Ok(TimeString(time as u64)),
            Err(e) => Err(E::custom(e.to_string())),
        }
    }

    fn visit_i64<E>(self, num: i64) -> StdResult<TimeString, E> {
        Ok(TimeString(num as u64))
    }
}

impl<'de> Deserialize<'de> for TimeString {
    fn deserialize<D: Deserializer<'de>>(
        deserializer: D,
    ) -> StdResult<TimeString, D::Error> {
        deserializer.deserialize_any(TimeStringVisitor)
    }
}


#[cfg(test)]
mod tests {
    use super::parse_time;


    #[test]
    fn test_parse_time() {
        // Success - simple
        assert_eq!(parse_time("25").unwrap(), 25);
        assert_eq!(parse_time("0").unwrap(), 0);
        assert_eq!(parse_time("").unwrap(), 0);

        // Success - complex
        assert_eq!(parse_time("10d11h6s").unwrap(), 903606);
        assert_eq!(parse_time("1d1d1d").unwrap(), 259200);

        // Failure
        assert!(parse_time("10q").is_err());
        assert!(parse_time("h").is_err());
    }
}
