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

use requests::{Request, RequestType};
use errors::{FisherResult, ErrorKind};


lazy_static! {
    static ref GITLAB_EVENTS: Vec<&'static str> = vec![
        "Push", "Tag Push", "Issue", "Note", "Merge Request", "Wiki Page",
        "Build", "Pipeline", "Confidential Issue",
    ];

    static ref GITLAB_HEADERS: Vec<&'static str> = vec![
        "X-Gitlab-Event",
    ];
}


#[derive(RustcDecodable)]
struct Config {
    secret: Option<String>,
    events: Option<Vec<String>>,
}


pub fn check_config(input: &str) -> FisherResult<()> {
    let config: Config = try!(json::decode(input));

    // Check the validity of the events
    if let Some(events) = config.events {
        // Check if the events exists
        for event in &events {
            if ! GITLAB_EVENTS.contains(&event.as_ref()) {
                // Return an error if the event doesn't exist
                return Err(ErrorKind::InvalidInput(format!(
                    r#""{}" is not a GitLab event"#, event
                )).into());
            }
        }
    }

    Ok(())
}


pub fn request_type(_req: &Request, _config: &str) -> RequestType {
    // GitLab provides no way to check if this is a ping :(
    RequestType::ExecuteHook
}


pub fn validate(req: &Request, config: &str) -> bool {
    let config: Config = json::decode(config).unwrap();

    // Check if the correct headers are provided
    for header in GITLAB_HEADERS.iter() {
        if ! req.headers.contains_key(*header) {
            return false;
        }
    }

    // Check if the secret token is correct
    if let Some(ref secret) = config.secret {
        // The header with the token must be present
        if let Some(token) = req.headers.get("X-Gitlab-Token") {
            // The token must match
            if token != secret {
                return false;
            }
        } else {
            return false;
        }
    }

    let event = normalize_event_name(
        req.headers.get("X-Gitlab-Event").unwrap()
    );

    // Check if the event should be accepted
    if let Some(ref events) = config.events {
        // The event is whitelisted
        if ! events.contains(&event.to_string()) {
            return false;
        }
    }

    // Check if the JSON body is valid
    if ! Json::from_str(&req.body).is_ok() {
        return false;
    }

    true
}


pub fn env(req: &Request, _config: &str) -> HashMap<String, String> {
    // Get the current event name
    let event_header = normalize_event_name(
        &req.headers.get("X-Gitlab-Event").unwrap()
    );

    let mut res = HashMap::new();
    res.insert("EVENT".to_string(), event_header.to_string());

    res
}


fn normalize_event_name(input: &str) -> &str {
    // Strip the ending " Hook"
    if input.ends_with(" Hook") {
        let split: Vec<&str> = input.rsplitn(2, " ").collect();

        return split.get(1).unwrap();
    }

    input
}


#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use utils::testing::*;
    use requests::{Request, RequestType};

    use super::{GITLAB_EVENTS, check_config, request_type, validate, env,
                normalize_event_name};


    #[test]
    fn test_check_config() {
        // Check for right configuration
        for right in &[
            r#"{}"#,
            r#"{"secret": "abcde"}"#,
            r#"{"events": ["Push", "Issue"]}"#,
            r#"{"secret": "abcde", "events": ["Push", "Issue"]}"#,
        ] {
            assert!(check_config(right).is_ok(), right.to_string());
        }

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
        for event in GITLAB_EVENTS.iter() {
            let mut request = dummy_request();
            request.headers.insert(
                "X-Gitlab-Event".to_string(),
                format!("{} Hook", event),
            );

            assert_eq!(request_type(&request, ""), RequestType::ExecuteHook);
        }
    }


    #[test]
    fn test_validate_basic() {
        // Check with a dummy request - missing headers and no json body
        assert!(! validate(&dummy_request(), "{}"));

        // Check with a request with the headers and no json body
        let mut req = dummy_request();
        req.headers.insert(
            "X-Gitlab-Event".to_string(), "Push Hook".to_string()
        );
        assert!(! validate(&req, "{}"));

        // Check with a request with missing headers and a JSON body
        let mut req = dummy_request();
        req.body = r#"{"a": "b"}"#.to_string();
        assert!(! validate(&req, "{}"));

        // Check with a request with the headers and a JSON body
        let mut req = dummy_request();
        req.headers.insert(
            "X-Gitlab-Event".to_string(), "Push Hook".to_string()
        );
        req.body = r#"{"a": "b"}"#.to_string();
        assert!(validate(&req, "{}"));
    }


    #[test]
    fn test_validate_secret() {
        let config = r#"{"secret": "abcde"}"#;
        let base_request = {
            let mut base = dummy_request();

            base.headers.insert(
                "X-Gitlab-Event".to_string(), "Push Hook".to_string()
            );
            base.body = r#"{"a": "b"}"#.to_string();

            base
        };

        // Make sure the base request validates without a secret
        assert!(validate(&base_request.clone(), "{}"));

        // Check a request without the header
        assert!(! validate(&base_request.clone(), config));

        // Check a request with the header but a wrong token
        let mut req = base_request.clone();
        req.headers.insert("X-Gitlab-Token".to_string(), "12345".to_string());
        assert!(! validate(&req, config));

        // Check a request with the header
        let mut req = base_request.clone();
        req.headers.insert("X-Gitlab-Token".to_string(), "abcde".to_string());
        assert!(validate(&req, config));
    }


    #[test]
    fn test_validate_events() {
        let config = r#"{"events": ["Push", "Issue"]}"#;
        fn with_event(name: &str) -> Request {
            let mut base = dummy_request();
            base.body = "{}".to_string();
            base.headers.insert(
                "X-Gitlab-Event".to_string(), name.to_string()
            );

            base
        }

        // With a list of allowed events
        assert!(validate(&with_event("Push Hook"), config));
        assert!(! validate(&with_event("Build Hook"), config));

        // Without a list of allowed events
        assert!(validate(&with_event("Push Hook"), "{}"));
        assert!(validate(&with_event("Build Hook"), "{}"));
        assert!(validate(&with_event("Strange Hook"), "{}"));
    }


    #[test]
    fn test_env() {
        let mut expected = HashMap::new();
        expected.insert("EVENT".to_string(), "Push".to_string());

        let mut req = dummy_request();
        req.headers.insert(
            "X-Gitlab-Event".to_string(), "Push Hook".to_string()
        );

        assert_eq!(env(&req, ""), expected);
    }


    #[test]
    fn test_normalize_event_name() {
        assert_eq!(normalize_event_name("Push"), "Push");
        assert_eq!(normalize_event_name("Push Hook"), "Push");
        assert_eq!(normalize_event_name("Push Hook Hook"), "Push Hook");
    }
}
