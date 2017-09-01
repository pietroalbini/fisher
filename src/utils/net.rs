// Copyright (C) 2016-2017 Pietro Albini
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
use std::net::IpAddr;

use common::prelude::*;


pub type Headers = HashMap<String, String>;


pub fn parse_forwarded_for(headers: &Headers) -> Result<Vec<IpAddr>> {
    let mut result = vec![];

    if let Some(header) = headers.get("X-Forwarded-For".into()) {
        // Parse the header content
        let splitted: Vec<&str> = header.split(',').collect();

        // Convert everything to instances of IpAddr
        for address in &splitted {
            result.push(address.trim().parse::<IpAddr>()?);
        }
    }

    Ok(result)
}


#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use super::{Headers, parse_forwarded_for};


    #[test]
    fn test_parse_forwarded_for() {
        // Test with no headers
        assert_eq!(
            parse_forwarded_for(&Headers::new()).unwrap(),
            Vec::<IpAddr>::new()
        );

        // Test with a single IP address
        let mut headers = Headers::new();
        headers.insert("X-Forwarded-For".into(), "127.0.0.1".into());
        assert_eq!(
            parse_forwarded_for(&headers).unwrap(),
            vec!["127.0.0.1".parse::<IpAddr>().unwrap()]
        );

        // Test with multiple IP addresses
        let mut headers = Headers::new();
        headers.insert(
            "X-Forwarded-For".into(),
            "127.0.0.1, 10.0.0.1".into()
        );
        assert_eq!(
            parse_forwarded_for(&headers).unwrap(),
            vec![
                "127.0.0.1".parse::<IpAddr>().unwrap(),
                "10.0.0.1".parse::<IpAddr>().unwrap()
            ]
        );

        // Test with a non-IP address
        let mut headers = Headers::new();
        headers.insert(
            "X-Forwarded-For".into(),
            "127.0.0.1, hey, 10.0.0.1".into()
        );
        assert!(parse_forwarded_for(&headers).is_err());
    }
}
