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

use rustc_serialize::json;

use processor::Request;
use errors::FisherResult;


#[derive(RustcDecodable)]
struct Config {
    secret: String,
}


pub fn check_config(input: String) -> FisherResult<()> {
    try!(json::decode::<Config>(&input));

    Ok(())
}


pub fn validate(req: Request, config: String) -> bool {
    let config: Config = json::decode(&config).unwrap();

    let secret;
    if let Some(found) = req.params.get("secret") {
        // Secret in the request parameters
        secret = found;
    } else if let Some(found) = req.headers.get("X-Fisher-Secret") {
        // Secret in the HTTP headers
        secret = found;
    } else {
        // No secret present, abort!
        return false;
    }

    // Abort if the secret doesn't match
    if secret != &config.secret {
        return false;
    }

    true
}


pub fn env(_config: String) -> HashMap<String, String> {
    HashMap::new()
}


#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::collections::HashMap;
    use std::net::{IpAddr, SocketAddr};

    use super::{check_config, validate, env};
    use processor;

    #[test]
    fn test_check_config() {
        // Check if valid config is accepted
        assert!(check_config(r#"{"secret":"abcde"}"#.to_string()).is_ok());

        let wrong = vec![
            // Empty configuration
            r#"{}"#,

            // Mispelled keys
            r#"{"secrt": "abcde"}"#,

            // Wrong types
            r#"{"secret": 123}"#,
            r#"{"secret": true}"#,
            r#"{"secret": ["a", "b"]}"#,
            r#"{"secret": {"a": "b"}}"#,
        ];
        for one in &wrong {
            assert!(check_config(one.to_string()).is_err());
        }
    }

    #[test]
    fn test_validate() {
        let config = r#"{"secret": "abcde"}"#;
        let base_request = processor::Request {
            headers: HashMap::new(),
            params: HashMap::new(),
            source: SocketAddr::new(
                IpAddr::from_str("127.0.0.1").unwrap(), 80
            ),
        };

        // Test a request with no headers or params
        // It should not be validated
        assert!(! validate(base_request.clone(), config.to_string()));

        // Test a request with the secret param, but the wrong secret key
        // It should not be validated
        let mut req = base_request.clone();
        req.params.insert("secret".to_string(), "12345".to_string());
        assert!(! validate(req, config.to_string()));

        // Test a request with the secret param and the correct secret key
        // It should be validated
        let mut req = base_request.clone();
        req.params.insert("secret".to_string(), "abcde".to_string());
        assert!(validate(req, config.to_string()));

        // Test a request with the secret header, but the wrong secret key
        // It should not be validated
        let mut req = base_request.clone();
        req.headers.insert("X-Fisher-Secret".to_string(), "12345".to_string());
        assert!(! validate(req, config.to_string()));

        // Test a request with the secret header and the correct secret key
        // It should be validated
        let mut req = base_request.clone();
        req.headers.insert("X-Fisher-Secret".to_string(), "abcde".to_string());
        assert!(validate(req, config.to_string()));
    }

    #[test]
    fn test_env() {
        let config = r#"{"secret": "abcde"}"#;
        let base_request = processor::Request {
            headers: HashMap::new(),
            params: HashMap::new(),
            source: SocketAddr::new(
                IpAddr::from_str("127.0.0.1").unwrap(), 80
            ),
        };

        // The environment must always be empty
        assert!(env(config.to_string()) == HashMap::new());
    }

}
