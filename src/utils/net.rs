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

use std::net::IpAddr;

use hyper::header::Headers;

use errors::FisherResult;


pub fn parse_forwarded_for(headers: &Headers) -> FisherResult<Vec<IpAddr>> {
    let mut result = vec![];

    if let Some(ref header) = headers.get_raw("X-Forwarded-For") {
        // Get only the first header of them
        if let Some(ref content) = header.get(0) {
            // Parse the header content
            let data = String::from_utf8_lossy(content).to_string();
            let splitted: Vec<&str> = data.split(",").collect();

            // Convert everything to instances of IpAddr
            for address in &splitted {
                result.push(try!(address.trim().parse::<IpAddr>()));
            }
        }
    }

    Ok(result)
}


#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use hyper::header::Headers;

    use super::parse_forwarded_for;


    #[test]
    fn test_parse_forwarded_for() {
        // Test with no headers
        assert_eq!(parse_forwarded_for(&Headers::new()).unwrap(), vec![]);

        // Test with a single IP address
        let mut headers = Headers::new();
        headers.set_raw("X-Forwarded-For", vec![b"127.0.0.1".to_vec()]);
        assert_eq!(
            parse_forwarded_for(&headers).unwrap(),
            vec!["127.0.0.1".parse::<IpAddr>().unwrap()]
        );

        // Test with multiple IP addresses
        let mut headers = Headers::new();
        headers.set_raw(
            "X-Forwarded-For",
            vec![b"127.0.0.1, 10.0.0.1".to_vec()]
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
        headers.set_raw(
            "X-Forwarded-For",
            vec![b"127.0.0.1, hey, 10.0.0.1".to_vec()]
        );
        assert!(parse_forwarded_for(&headers).is_err());
    }
}
