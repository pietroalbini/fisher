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

use rustc_serialize::json;

use providers::prelude::*;


#[derive(Debug, Clone, RustcDecodable)]
pub struct StandaloneProvider {
    secret: String,

    param_name: Option<String>,
    header_name: Option<String>,
}

impl StandaloneProvider {

    fn param_name(&self) -> String {
        match self.param_name {
            Some(ref name) => name.clone(),
            None => "secret".into(),
        }
    }

    fn header_name(&self) -> String {
        match self.header_name {
            Some(ref name) => name.clone(),
            None => "X-Fisher-Secret".into(),
        }
    }
}

impl Provider for StandaloneProvider {

    fn new(config: &str) -> FisherResult<Self> {
        // Check if it's possible to create a new instance and return it
        let inst = try!(json::decode(config));
        Ok(inst)
    }

    fn validate(&self, req: &Request) -> RequestType {
        // First of all check the secret code
        let secret;
        if let Some(found) = req.params.get(&self.param_name()) {
            // Secret in the request parameters
            secret = found;
        } else if let Some(found) = req.headers.get(&self.header_name()) {
            // Secret in the HTTP headers
            secret = found;
        } else {
            // No secret present, abort!
            return RequestType::Invalid;
        }

        // Abort if the secret doesn't match
        if *secret != self.secret {
            return RequestType::Invalid;
        }

        RequestType::ExecuteHook
    }

    fn env(&self, _req: &Request) -> HashMap<String, String> {
        HashMap::new()
    }
}


#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use utils::testing::*;
    use requests::RequestType;
    use providers::Provider;

    use super::StandaloneProvider;


    #[test]
    fn test_new() {
        // Check if valid config is accepted
        let right = vec![
            r#"{"secret": "abcde"}"#,
            r#"{"secret": "abcde", "param_name": "a"}"#,
            r#"{"secret": "abcde", "header_name": "X-b"}"#,
            r#"{"secret": "abcde", "param_name": "a", "header_name": "b"}"#,
        ];
        for one in &right {
            assert!(StandaloneProvider::new(one).is_ok());
        }

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
            assert!(StandaloneProvider::new(one).is_err());
        }
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
        let p = StandaloneProvider::new(config).unwrap();

        // Test a request with no headers or params
        // It should not be validate
        assert_eq!(p.validate(&dummy_request()), RequestType::Invalid);

        // Test a request with the secret param, but the wrong secret key
        // It should not be validated
        let mut req = dummy_request();
        req.params.insert(param_name.to_string(), "12345".to_string());
        assert_eq!(p.validate(&req), RequestType::Invalid);

        // Test a request with the secret param and the correct secret key
        // It should be validated
        let mut req = dummy_request();
        req.params.insert(param_name.to_string(), "abcde".to_string());
        assert_eq!(p.validate(&req), RequestType::ExecuteHook);

        // Test a request with the secret header, but the wrong secret key
        // It should not be validated
        let mut req = dummy_request();
        req.headers.insert(header_name.to_string(), "12345".to_string());
        assert_eq!(p.validate(&req), RequestType::Invalid);

        // Test a request with the secret header and the correct secret key
        // It should be validated
        let mut req = dummy_request();
        req.headers.insert(header_name.to_string(), "abcde".to_string());
        assert_eq!(p.validate(&req), RequestType::ExecuteHook);
    }

    #[test]
    fn test_env() {
        let p = StandaloneProvider::new(r#"{"secret": "abcde"}"#).unwrap();

        // The environment must always be empty
        assert!(p.env(&dummy_request()) == HashMap::new());
    }

}
