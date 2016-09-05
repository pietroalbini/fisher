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

use errors::{FisherResult, FisherError, ErrorKind};
use web::requests::{Request, RequestType};


pub fn check_config(config: &str) -> FisherResult<()> {
    // If the configuration is "yes", then it's correct
    if config != "FAIL" {
        Ok(())
    } else {
        // This error doesn't make any sense, but it's still an error
        Err(FisherError::new(
            ErrorKind::ProviderNotFound(String::new())
        ))
    }
}


pub fn request_type(req: &Request, _config: &str) -> RequestType {
    // Allow to override the result of this
    if let Some(request_type) = req.params.get("request_type") {
        match request_type.as_ref() {
            // "ping" will return RequestType::Ping
            "ping" => {
                return RequestType::Ping;
            },
            _ => {}
        }
    }

    // Return ExecuteHook anywhere else
    RequestType::ExecuteHook
}


pub fn validate(req: &Request, _config: &str) -> bool {
    // If the secret param is provided, validate it
    if let Some(secret) = req.params.get("secret") {
        if secret == "testing" {
            return true;
        } else {
            return false;
        }
    }

   true
}


pub fn env(_req: &Request, _config: &str) -> HashMap<String, String> {
    HashMap::new()
}


#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use providers::core::tests::dummy_request;
    use web::requests::RequestType;

    use super::{check_config, request_type, validate, env};


    #[test]
    fn test_check_config() {
        assert!(check_config("").is_ok());
        assert!(check_config("SOMETHING").is_ok());
        assert!(check_config("FAIL").is_err());
    }


    #[test]
    fn test_request_type() {
        // Without any special parameter return an ExecuteHook
        assert_eq!(
            request_type(&dummy_request(), ""),
            RequestType::ExecuteHook
        );

        // With the parameter but with no meaningful value
        let mut req = dummy_request();
        req.params.insert("request_type".to_string(), "something".to_string());
        assert_eq!(
            request_type(&req, ""),
            RequestType::ExecuteHook
        );

        // With the parameter and the "ping" value
        let mut req = dummy_request();
        req.params.insert("request_type".to_string(), "ping".to_string());
        assert_eq!(
            request_type(&req, ""),
            RequestType::Ping
        );
    }


    #[test]
    fn test_validate() {
        // Without any secret
        assert!(validate(&dummy_request(), ""));

        // With the wrong secret
        let mut req = dummy_request();
        req.params.insert("secret".to_string(), "wrong!!!".to_string());
        assert!(! validate(&req, ""));

        // With the correct secret
        let mut req = dummy_request();
        req.params.insert("secret".to_string(), "testing".to_string());
        assert!(validate(&req, ""));
    }


    #[test]
    fn test_env() {
        assert_eq!(env(&dummy_request(), ""), HashMap::new());
    }
}
