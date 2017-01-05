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

use rustc_serialize::json::{self, Json};
use rustc_serialize::hex::FromHex;
use ring;

use providers::prelude::*;
use errors::ErrorKind;


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


#[derive(Debug, Clone, RustcDecodable)]
pub struct GitHubProvider {
    secret: Option<String>,
    events: Option<Vec<String>>,
}

impl ProviderTrait for GitHubProvider {

    fn new(input: &str) -> FisherResult<GitHubProvider> {
        let inst: GitHubProvider = try!(json::decode(input));

        if let Some(ref events) = inst.events {
            // Check if the events exists
            for event in events {
                if ! GITHUB_EVENTS.contains(&event.as_ref()) {
                    // Return an error if the event doesn't exist
                    return Err(ErrorKind::InvalidInput(format!(
                        r#""{}" is not a GitHub event"#, event
                    )).into());
                }
            }
        }

        Ok(inst)
    }

    fn validate(&self, req: &Request) -> RequestType {
        // Check if the correct headers are present
        for header in GITHUB_HEADERS.iter() {
            if ! req.headers.contains_key(*header) {
                return RequestType::Invalid;
            }
        }

        // Check the signature only if a secret key was provided
        if let Some(ref secret) = self.secret {
            // Check if the signature is valid
            let signature = req.headers.get("X-Hub-Signature").unwrap();
            if ! verify_signature(secret, &req.body, &signature) {
                return RequestType::Invalid;
            }
        }

        // Check if the event is valid
        let event = req.headers.get("X-GitHub-Event").unwrap();
        if !(
            GITHUB_EVENTS.contains(&event.as_ref())
            || *event == "ping".to_string()
        ) {
            return RequestType::Invalid;
        }

        // Check if the event should be accepted
        if let Some(ref events) = self.events {
            if ! events.contains(&event) {
                return RequestType::Invalid;
            }
        }

        // Check if the JSON in the body is valid
        if ! Json::from_str(&req.body).is_ok() {
            return RequestType::Invalid;
        }

        // The "ping" event is a ping (doh!)
        if event == "ping" {
            return RequestType::Ping;
        }

        // Process the hook in the other cases
        RequestType::ExecuteHook
    }

    fn env(&self, req: &Request) -> HashMap<String, String> {
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
    use utils::testing::*;
    use requests::RequestType;
    use providers::ProviderTrait;

    use super::{GITHUB_EVENTS, GitHubProvider, verify_signature};


    #[test]
    fn test_new() {
        // Check for right configurations
        for right in &[
            r#"{}"#,
            r#"{"secret": "abcde"}"#,
            r#"{"events": ["push", "fork"]}"#,
            r#"{"secret": "abcde", "events": ["push", "fork"]}"#,
        ] {
            assert!(GitHubProvider::new(right).is_ok(), right.to_string());
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
            assert!(GitHubProvider::new(wrong).is_err(), wrong.to_string());
        }
    }


    #[test]
    fn test_request_type() {
        let provider = GitHubProvider::new("{}").unwrap();

        // This helper gets the request type of an event
        macro_rules! assert_req_type {
            ($provider:expr, $event:expr, $expected:expr) => {
                let mut request = dummy_request();
                let _ = request.headers.insert(
                    "X-GitHub-Event".into(),
                    $event.to_string(),
                );
                let _ = request.headers.insert(
                    "X-GitHub-Delivery".into(),
                    "12345".into(),
                );
                let _ = request.headers.insert(
                    "X-Hub-Signature".into(),
                    "invalid".into(),
                );
                request.body = "{}".into();

                assert_eq!($provider.validate(&request), $expected);
            };
        }

        assert_req_type!(provider, "ping", RequestType::Ping);
        for event in GITHUB_EVENTS.iter() {
            assert_req_type!(provider, event, RequestType::ExecuteHook);
        }
    }


    #[test]
    fn test_env() {
        let provider = GitHubProvider::new("{}").unwrap();

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
        let env = provider.env(&request);

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
