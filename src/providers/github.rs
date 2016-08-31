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

use rustc_serialize::json::{self, Json};
use rustc_serialize::hex::FromHex;
use ring;

use processor::{Request, RequestType};
use errors::{FisherError, ErrorKind, FisherResult};


lazy_static! {
    static ref GITHUB_EVENTS: Vec<&'static str> = vec![
        "commit_comment", "create", "delete", "deployment",
        "deployment_status", "fork", "gollum", "issue_comment", "issues",
        "member", "membership", "page_build", "public",
        "pull_reques_review_comment", "pull_request", "push", "repository",
        "release", "status", "team_add", "watch",
    ];

    static ref GITHUB_HEADERS: Vec<&'static str> = vec![
        "X-GitHub-Event",
        "X-Hub-Signature",
        "X-GitHub-Delivery",
    ];
}


#[derive(RustcDecodable)]
struct Config {
    secret: Option<String>,
    events: Option<Vec<String>>,
}


pub fn check_config(input: &str) -> FisherResult<()> {
    let config: Config = try!(json::decode(input));

    if let Some(events) = config.events {
        // Check if the events exists
        for event in &events {
            if ! GITHUB_EVENTS.contains(&event.as_ref()) {
                // Return an error if the event doesn't exist
                return Err(FisherError::new(
                    ErrorKind::ProviderConfigError(format!(
                        "Invalid GitHub event: {}", event
                    ))
                ));
            }
        }
    }

    Ok(())
}


pub fn request_type(req: &Request, _config: &str) -> RequestType {
    // The X-GitHub-Event contains the event type
    if let Some(event) = req.headers.get(&"X-GitHub-Event".to_string()) {
        // The "ping" event is a ping (doh!)
        if event == "ping" {
            return RequestType::Ping;
        }
    }

    // Process the hook in the other cases
    RequestType::ExecuteHook
}


pub fn validate(req: &Request, config: &str) -> bool {
    let config: Config = json::decode(config).unwrap();

    // Check if the correct headers are present
    for header in GITHUB_HEADERS.iter() {
        if ! req.headers.contains_key(*header) {
            return false;
        }
    }

    // Check the signature only if a secret key was provided
    if let Some(ref secret) = config.secret {
        // Check if the signature is valid
        let signature = req.headers.get("X-Hub-Signature").unwrap();
        if ! verify_signature(secret, &req.body, &signature) {
            return false;
        }
    }

    // Check if the event is valid
    let event = req.headers.get("X-GitHub-Event").unwrap();
    if ! GITHUB_EVENTS.contains(&event.as_ref()) {
        return false;
    }

    // Check if the event should be accepted
    if let Some(ref events) = config.events {
        if ! events.contains(&event) {
            return false;
        }
    }

    // Check if the JSON in the body is valid
    if ! Json::from_str(&req.body).is_ok() {
        return false;
    }

    true
}


pub fn env(req: &Request, _config: &str) -> HashMap<String, String> {
    let mut res = HashMap::new();

    res.insert(
        "EVENT".to_string(),
        req.headers.get("X-GitHub-Event").unwrap().clone()
    );

    res.insert(
        "DELIVERY_ID".to_string(),
        req.headers.get("X-GitHub-Delivery").unwrap().clone()
    );

    res
}


fn verify_signature(secret: &str, payload: &str, raw_signature: &str) -> bool {
    // The signature must have a =
    if ! raw_signature.contains("=") {
        return false;
    }

    // Split the raw signature to get the algorithm and the signature
    let splitted: Vec<&str> = raw_signature.split("=").collect();
    let algorithm = splitted.get(0).unwrap();
    let hex_signature = splitted.iter().skip(1).map(|i| *i)
                                .collect::<Vec<&str>>().join("=");

    // Convert the signature from hex
    let signature;
    if let Ok(converted) = hex_signature.from_hex() {
        signature = converted;
    } else {
        // This is not hex
        return false;
    }

    // Get the correct digest
    let digest = match *algorithm {
        "sha1" => &ring::digest::SHA1,
        _ => {
            // Unknown digest, return false
            return false;
        },
    };

    // Verify the HMAC signature
    let key = ring::hmac::VerificationKey::new(&digest, secret.as_bytes());
    ring::hmac::verify(&key, payload.as_bytes(), &signature).is_ok()
}


#[cfg(test)]
mod tests {
    use super::{GITHUB_EVENTS, check_config, request_type, env,
                verify_signature};
    use processor::RequestType;
    use providers::core::tests::dummy_request;


    #[test]
    fn test_check_config() {
        // Check for right configurations
        for right in &[
            r#"{}"#,
            r#"{"secret": "abcde"}"#,
            r#"{"events": ["push", "fork"]}"#,
            r#"{"secret": "abcde", "events": ["push", "fork"]}"#,
        ] {
            assert!(check_config(right).is_ok(), right.to_string());
        }

        // Checks for wrong configurations
        for wrong in &[
            // Wrong types
            r#"{"secret": 12345}"#,
            r#"{"secret": true}"#,
            r#"{"events": 12345}"#,
            r#"{"events": true}"#,
            r#"{"events": {}}"#,
            r#"{"events": [12345]}"#,
            r#"{"events": [true]}"#,
            r#"{"events": ["invalid_event"]}"#,
        ] {
            assert!(check_config(wrong).is_err(), wrong.to_string());
        }
    }


    #[test]
    fn test_request_type() {
        // This helper gets the request type of an event
        fn get_request_type(event: &str) -> RequestType {
            let mut request = dummy_request();
            request.headers.insert(
                "X-GitHub-Event".to_string(),
                event.to_string()
            );

            request_type(&request, "")
        }

        assert_eq!(get_request_type("ping"), RequestType::Ping);
        for event in GITHUB_EVENTS.iter() {
            assert_eq!(get_request_type(event), RequestType::ExecuteHook);
        }
    }


    #[test]
    fn test_env() {
        // Create a dummy request
        let mut request = dummy_request();
        request.headers.insert(
            "X-GitHub-Event".to_string(),
            "ping".to_string()
        );
        request.headers.insert(
            "X-GitHub-Delivery".to_string(),
            "12345".to_string()
        );

        // Get the env
        let env = env(&request, "");

        assert_eq!(env.len(), 2);
        assert_eq!(*env.get("EVENT").unwrap(), "ping".to_string());
        assert_eq!(*env.get("DELIVERY_ID").unwrap(), "12345".to_string());
    }


    #[test]
    fn test_verify_signature() {
        // Check if the function allows invalid signatures
        for signature in &[
            "invalid",  // No algorithm
            "invalid=invalid",  // Invalid algorithm
            "sha1=g",  // The signature is not hex

            // Invalid signature (the first "e" should be "f")
            "sha1=e75efc0f29bf50c23f99b30b86f7c78fdaf5f11d",
        ] {
            assert!(
                ! verify_signature("secret", "payload", signature),
                signature.to_string()
            );
        }

        // This is known to be right
        assert!(verify_signature(
            "secret", "payload",
            "sha1=f75efc0f29bf50c23f99b30b86f7c78fdaf5f11d"
        ));
    }
}
