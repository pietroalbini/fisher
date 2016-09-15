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

use requests::{Request, RequestType};
use errors::FisherResult;


#[derive(RustcDecodable)]
struct Config {
    secret: String,

    param_name: Option<String>,
    header_name: Option<String>,
}

impl Config {

    fn param_name(&self) -> &str {
        match self.param_name {
            Some(ref name) => name,
            None => "secret",
        }
    }

    fn header_name(&self) -> &str {
        match self.header_name {
            Some(ref name) => name,
            None => "X-Fisher-Secret",
        }
    }
}


pub fn check_config(input: &str) -> FisherResult<()> {
    try!(json::decode::<Config>(input));

    Ok(())
}


pub fn request_type(_req: &Request, _config: &str) -> RequestType {
    // This provider supports only RequestType::ExecuteHook
    RequestType::ExecuteHook
}


pub fn validate(req: &Request, config: &str) -> bool {
    let config: Config = json::decode(config).unwrap();

    let secret;
    if let Some(found) = req.params.get(config.param_name()) {
        // Secret in the request parameters
        secret = found;
    } else if let Some(found) = req.headers.get(config.header_name()) {
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


pub fn env(_req: &Request, _config: &str) -> HashMap<String, String> {
    HashMap::new()
}


#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use utils::testing::*;
    use requests::RequestType;

    use super::{check_config, request_type, validate, env};


    #[test]
    fn test_check_config() {
        // Check if valid config is accepted
        assert!(check_config(r#"{"secret":"abcde"}"#).is_ok());

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
            assert!(check_config(one).is_err());
        }
    }

    #[test]
    fn test_request_type() {
        let config = r#"{"secret": "abcde"}"#;

        assert_eq!(
            request_type(&dummy_request(), &config),
            RequestType::ExecuteHook
        );
    }

    #[test]
    fn test_validate() {
        let config = r#"{"secret": "abcde"}"#;
        let config_custom = concat!(
            r#"{"secret": "abcde", "param_name": "a","#,
            r#" "header_name": "X-A"}"#
        );

        test_validate_inner(config, "secret", "X-Fisher-Secret");
        test_validate_inner(config_custom, "a", "X-A");
    }

    fn test_validate_inner(config: &str, param_name: &str, header_name: &str) {
        // Test a request with no headers or params
        // It should not be validate
        assert!(! validate(&dummy_request(), config));

        // Test a request with the secret param, but the wrong secret key
        // It should not be validated
        let mut req = dummy_request();
        req.params.insert(param_name.to_string(), "12345".to_string());
        assert!(! validate(&req, config));

        // Test a request with the secret param and the correct secret key
        // It should be validated
        let mut req = dummy_request();
        req.params.insert(param_name.to_string(), "abcde".to_string());
        assert!(validate(&req, config));

        // Test a request with the secret header, but the wrong secret key
        // It should not be validated
        let mut req = dummy_request();
        req.headers.insert(header_name.to_string(), "12345".to_string());
        assert!(! validate(&req, config));

        // Test a request with the secret header and the correct secret key
        // It should be validated
        let mut req = dummy_request();
        req.headers.insert(header_name.to_string(), "abcde".to_string());
        assert!(validate(&req, config));
    }

    #[test]
    fn test_env() {
        let config = r#"{"secret": "abcde"}"#;

        // The environment must always be empty
        assert!(env(&dummy_request(), config) == HashMap::new());
    }

}
