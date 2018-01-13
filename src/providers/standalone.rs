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

use std::net::IpAddr;

use serde_json;

use providers::prelude::*;


#[derive(Debug, Deserialize)]
pub struct StandaloneProvider {
    secret: Option<String>,
    from: Option<Vec<IpAddr>>,

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

impl ProviderTrait for StandaloneProvider {
    fn new(config: &str) -> Result<Self> {
        // Check if it's possible to create a new instance and return it
        let inst = serde_json::from_str(config)?;
        Ok(inst)
    }

    fn validate(&self, request: &Request) -> RequestType {
        let req;
        if let Request::Web(ref inner) = *request {
            req = inner;
        } else {
            return RequestType::Invalid;
        }

        // Check if the secret code is valid
        if let Some(ref correct_secret) = self.secret {
            let secret = if let Some(found) = req.params.get(&self.param_name()) {
                // Secret in the request parameters
                found
            } else if let Some(found) = req.headers.get(&self.header_name()) {
                // Secret in the HTTP headers
                found
            } else {
                // No secret present, abort!
                return RequestType::Invalid;
            };

            // Abort if the secret doesn't match
            if secret != correct_secret {
                return RequestType::Invalid;
            }
        }

        // Check if the IP address is allowed
        if let Some(ref allowed) = self.from {
            if !allowed.contains(&req.source) {
                return RequestType::Invalid;
            }
        }

        RequestType::ExecuteHook
    }

    fn build_env(&self, _: &Request, _: &mut EnvBuilder) -> Result<()> {
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use utils::testing::*;
    use requests::RequestType;
    use providers::ProviderTrait;
    use scripts::EnvBuilder;

    use super::StandaloneProvider;


    #[test]
    fn test_new() {
        // Check if valid config is accepted
        let right = vec![
            r#"{}"#,
            r#"{"secret": "abcde"}"#,
            r#"{"secret": "abcde", "param_name": "a"}"#,
            r#"{"secret": "abcde", "header_name": "X-b"}"#,
            r#"{"secret": "abcde", "param_name": "a", "header_name": "b"}"#,
            r#"{"from": ["127.0.0.1", "192.168.1.1", "10.0.0.2"]}"#,
            r#"{"from": ["127.0.0.1"], "secret": "abcde"}"#,
        ];
        for one in &right {
            assert!(StandaloneProvider::new(one).is_ok(), "Should be valid: {}", one);
        }

        let wrong = vec![
            r#"{"secret": 123}"#,
            r#"{"secret": true}"#,
            r#"{"secret": ["a", "b"]}"#,
            r#"{"secret": {"a": "b"}}"#,
            r#"{"from": "127.0.0.1"}"#,
            r#"{"from": ["256.0.0.1"]}"#,
        ];
        for one in &wrong {
            assert!(StandaloneProvider::new(one).is_err(), "Should be invalid: {}", one);
        }
    }

    #[test]
    fn test_validate_secret() {
        let config = r#"{"secret": "abcde"}"#;
        let config_custom = concat!(
            r#"{"secret": "abcde", "param_name": "a","#,
            r#" "header_name": "X-A"}"#
        );

        test_validate_inner_secret(config, "secret", "X-Fisher-Secret");
        test_validate_inner_secret(config_custom, "a", "X-A");
    }

    fn test_validate_inner_secret(config: &str, param_name: &str, header_name: &str) {
        let p = StandaloneProvider::new(config).unwrap();

        // Test a request with no headers or params
        // It should not be validate
        assert_eq!(
            p.validate(&dummy_web_request().into()),
            RequestType::Invalid
        );

        // Test a request with the secret param, but the wrong secret key
        // It should not be validated
        let mut req = dummy_web_request();
        req.params
            .insert(param_name.to_string(), "12345".to_string());
        assert_eq!(p.validate(&req.into()), RequestType::Invalid);

        // Test a request with the secret param and the correct secret key
        // It should be validated
        let mut req = dummy_web_request();
        req.params
            .insert(param_name.to_string(), "abcde".to_string());
        assert_eq!(p.validate(&req.into()), RequestType::ExecuteHook);

        // Test a request with the secret header, but the wrong secret key
        // It should not be validated
        let mut req = dummy_web_request();
        req.headers
            .insert(header_name.to_string(), "12345".to_string());
        assert_eq!(p.validate(&req.into()), RequestType::Invalid);

        // Test a request with the secret header and the correct secret key
        // It should be validated
        let mut req = dummy_web_request();
        req.headers
            .insert(header_name.to_string(), "abcde".to_string());
        assert_eq!(p.validate(&req.into()), RequestType::ExecuteHook);
    }

    #[test]
    fn test_validate_from() {
        let config = r#"{"from": ["192.168.1.1", "10.0.0.1"]}"#;
        let p = StandaloneProvider::new(config).unwrap();

        let mut req = dummy_web_request();
        req.source = "127.0.0.1".parse().unwrap();
        assert_eq!(p.validate(&req.into()), RequestType::Invalid);

        for ip in &["192.168.1.1", "10.0.0.1"] {
            let mut req = dummy_web_request();
            req.source = ip.parse().unwrap();
            assert_eq!(p.validate(&req.into()), RequestType::ExecuteHook);
        }
    }


    #[test]
    fn test_build_env() {
        let p = StandaloneProvider::new(r#"{"secret": "abcde"}"#).unwrap();
        let mut b = EnvBuilder::dummy();
        p.build_env(&dummy_web_request().into(), &mut b).unwrap();

        assert_eq!(b.dummy_data().env, HashMap::new());
        assert_eq!(b.dummy_data().files, HashMap::new());
    }
}
