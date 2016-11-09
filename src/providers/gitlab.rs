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

use rustc_serialize::json::{self, Json};

use providers::prelude::*;
use errors::ErrorKind;


lazy_static! {
    static ref GITLAB_EVENTS: Vec<&'static str> = vec![
        "Push", "Tag Push", "Issue", "Note", "Merge Request", "Wiki Page",
        "Build", "Pipeline", "Confidential Issue",
    ];

    static ref GITLAB_HEADERS: Vec<&'static str> = vec![
        "X-Gitlab-Event",
    ];
}


#[derive(Clone, RustcDecodable)]
pub struct GitLabProvider {
    secret: Option<String>,
    events: Option<Vec<String>>,
}

impl Provider for GitLabProvider {

    fn new(config: &str) -> FisherResult<Self> {
        let inst: GitLabProvider = try!(json::decode(config));

        // Check the validity of the events
        if let Some(ref events) = inst.events {
            // Check if the events exists
            for event in events {
                if ! GITLAB_EVENTS.contains(&event.as_ref()) {
                    // Return an error if the event doesn't exist
                    return Err(ErrorKind::InvalidInput(format!(
                        r#""{}" is not a GitLab event"#, event
                    )).into());
                }
            }
        }

        Ok(inst)
    }

    fn validate(&self, req: &Request) -> RequestType {
        // Check if the correct headers are provided
        for header in GITLAB_HEADERS.iter() {
            if ! req.headers.contains_key(*header) {
                return RequestType::Invalid;
            }
        }

        // Check if the secret token is correct
        if let Some(ref secret) = self.secret {
            // The header with the token must be present
            if let Some(token) = req.headers.get("X-Gitlab-Token") {
                // The token must match
                if token != secret {
                    return RequestType::Invalid;
                }
            } else {
                return RequestType::Invalid;
            }
        }

        let event = normalize_event_name(
            req.headers.get("X-Gitlab-Event").unwrap()
        );

        // Check if the event should be accepted
        if let Some(ref events) = self.events {
            // The event is whitelisted
            if ! events.contains(&event.to_string()) {
                return RequestType::Invalid;
            }
        }

        // Check if the JSON body is valid
        if ! Json::from_str(&req.body).is_ok() {
            return RequestType::Invalid;
        }

        RequestType::ExecuteHook
    }

    fn env(&self, req: &Request) -> HashMap<String, String> {
        // Get the current event name
        let event_header = normalize_event_name(
            &req.headers.get("X-Gitlab-Event").unwrap()
        );

        let mut res = HashMap::new();
        res.insert("EVENT".to_string(), event_header.to_string());

        res
    }
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
    use providers::Provider;

    use super::{GITLAB_EVENTS, GitLabProvider, normalize_event_name};


    fn base_request() -> Request {
        let mut base = dummy_request();

        base.headers.insert(
            "X-Gitlab-Event".to_string(), "Push Hook".to_string()
        );
        base.body = r#"{"a": "b"}"#.to_string();

        base
    }


    #[test]
    fn test_new() {
        // Check for right configuration
        for right in &[
            r#"{}"#,
            r#"{"secret": "abcde"}"#,
            r#"{"events": ["Push", "Issue"]}"#,
            r#"{"secret": "abcde", "events": ["Push", "Issue"]}"#,
        ] {
            assert!(GitLabProvider::new(right).is_ok(), right.to_string());
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
            assert!(GitLabProvider::new(wrong).is_err(), wrong.to_string());
        }
    }


    #[test]
    fn test_validate_request_type() {
        let provider = GitLabProvider::new("{}").unwrap();

        for event in GITLAB_EVENTS.iter() {
            let mut request = base_request();
            request.headers.insert(
                "X-Gitlab-Event".to_string(),
                format!("{} Hook", event),
            );

            assert_eq!(provider.validate(&request), RequestType::ExecuteHook);
        }
    }


    #[test]
    fn test_validate_basic() {
        let provider = GitLabProvider::new("{}").unwrap();

        // Check with a dummy request - missing headers and no json body
        assert_eq!(
            provider.validate(&dummy_request()),
            RequestType::Invalid
        );

        // Check with a request with the headers and no JSON body
        let mut req = dummy_request();
        req.headers.insert(
            "X-Gitlab-Event".to_string(), "Push Hook".to_string()
        );
        assert_eq!(
            provider.validate(&req),
            RequestType::Invalid
        );

        // Check with a request with missing headers and a JSON body
        let mut req = dummy_request();
        req.body = r#"{"a": "b"}"#.to_string();
        assert_eq!(
            provider.validate(&req),
            RequestType::Invalid
        );

        // Check with a request with the headers and a JSON body
        let mut req = dummy_request();
        req.headers.insert(
            "X-Gitlab-Event".to_string(), "Push Hook".to_string()
        );
        req.body = r#"{"a": "b"}"#.to_string();
        assert_eq!(
            provider.validate(&req),
            RequestType::ExecuteHook
        );
    }


    #[test]
    fn test_validate_secret() {
        let provider = GitLabProvider::new(r#"{"secret": "abcde"}"#).unwrap();

        // Make sure the base request validates without a secret
        let no_secret = GitLabProvider::new("{}").unwrap();
        assert_eq!(
            no_secret.validate(&base_request()),
            RequestType::ExecuteHook
        );

        // Check a request without the header
        assert_eq!(
            provider.validate(&base_request()),
            RequestType::Invalid
        );

        // Check a request with the header but a wrong token
        let mut req = base_request();
        req.headers.insert("X-Gitlab-Token".to_string(), "12345".to_string());
        assert_eq!(
            provider.validate(&req),
            RequestType::Invalid
        );

        // Check a request with the header
        let mut req = base_request();
        req.headers.insert("X-Gitlab-Token".to_string(), "abcde".to_string());
        assert_eq!(
            provider.validate(&req),
            RequestType::ExecuteHook
        );
    }


    #[test]
    fn test_validate_events() {
        let config = r#"{"events": ["Push", "Issue"]}"#;
        let provider = GitLabProvider::new(config).unwrap();

        fn with_event(name: &str) -> Request {
            let mut base = base_request();
            base.body = "{}".to_string();
            base.headers.insert(
                "X-Gitlab-Event".to_string(), name.to_string()
            );

            base
        }

        // With a list of allowed events
        assert_eq!(
            provider.validate(&with_event("Push Hook")),
            RequestType::ExecuteHook
        );
        assert_eq!(
            provider.validate(&with_event("Build Hook")),
            RequestType::Invalid
        );

        // Without a list of allowed events
        let provider = GitLabProvider::new("{}").unwrap();
        assert_eq!(
            provider.validate(&with_event("Push Hook")),
            RequestType::ExecuteHook
        );
        assert_eq!(
            provider.validate(&with_event("Build Hook")),
            RequestType::ExecuteHook
        );
        assert_eq!(
            provider.validate(&with_event("Strange Hook")),
            RequestType::ExecuteHook
        );
    }


    #[test]
    fn test_env() {
        let mut expected = HashMap::new();
        expected.insert("EVENT".to_string(), "Push".to_string());

        let mut req = base_request();
        req.headers.insert(
            "X-Gitlab-Event".to_string(), "Push Hook".to_string()
        );

        let provider = GitLabProvider::new("{}").unwrap();
        assert_eq!(provider.env(&req), expected);
    }


    #[test]
    fn test_normalize_event_name() {
        assert_eq!(normalize_event_name("Push"), "Push");
        assert_eq!(normalize_event_name("Push Hook"), "Push");
        assert_eq!(normalize_event_name("Push Hook Hook"), "Push Hook");
    }
}
