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
use errors::ErrorKind;
use jobs::JobOutput;


lazy_static! {
    static ref EVENTS: Vec<&'static str> = vec![
        "job_completed", "job_failed",
    ];
    static ref REQUIRED_ARGS: Vec<&'static str> = vec![
        "event", "hook_name",
    ];
}


#[derive(Clone, RustcDecodable)]
pub struct StatusProvider {
    events: Vec<String>,
    hooks: Option<Vec<String>>,
}

impl StatusProvider {

    #[inline]
    pub fn hook_allowed(&self, name: &String) -> bool {
        // Check if it's allowed only if a whitelist was provided
        if let Some(ref hooks) = self.hooks {
            if ! hooks.contains(name) {
                return false;
            }
        }

        true
    }

    #[inline]
    pub fn event_allowed(&self, name: &String) -> bool {
        self.events.contains(name)
    }
}

impl Provider for StatusProvider {

    fn new(config: &str) -> FisherResult<Self> {
        let inst: Self = try!(json::decode(config));

        for event in &inst.events {
            if ! EVENTS.contains(&event.as_ref()) {
                // Return an error if the event doesn't exist
                return Err(ErrorKind::InvalidInput(format!(
                    r#""{}" is not a FIsher status event"#, event
                )).into());
            }
        }

        Ok(inst)
    }

    fn validate(&self, req: &Request) -> RequestType {
        // There must be all (and only) the required parameters
        for param in req.params.keys() {
            if ! REQUIRED_ARGS.contains(&param.as_ref()) {
                return RequestType::Invalid;
            }
        }
        if req.params.len() != REQUIRED_ARGS.len() {
            return RequestType::Invalid;
        }

        // Events must exist
        let event = req.params.get("event").unwrap().as_str();
        if ! EVENTS.contains(&event) {
            return RequestType::Invalid;
        }

        // Event-specific validation
        match event {
            "job_completed" | "job_failed" => {
                // The request body must be a serialized JobOutput
                if json::decode::<JobOutput>(&req.body).is_err() {
                    return RequestType::Invalid;
                }
            }
            _ => unreachable!(),
        }

        // The hook name must be allowed
        if ! self.hook_allowed(req.params.get("hook_name").unwrap()) {
            return RequestType::Invalid;
        }

        // The event must be allowed
        if ! self.event_allowed(req.params.get("event").unwrap()) {
            return RequestType::Invalid;
        }

        // Requests for this provider are internal
        RequestType::Internal
    }

    fn env(&self, req: &Request) -> HashMap<String, String> {
        let mut env = HashMap::new();

        // Move all the params to the env
        for (key, value) in req.params.iter() {
            env.insert(key.to_uppercase(), value.clone());
        }

        // Event-specific env
        match req.params.get("event").unwrap().as_str() {
            "job_completed" | "job_failed" => {
                let data = json::decode::<JobOutput>(&req.body).unwrap();

                env.insert(
                    "SUCCESS".into(), if data.success {
                        "1"
                    } else { "0" }.into()
                );
                env.insert(
                    "EXIT_CODE".into(),
                    if let Some(code) = data.exit_code {
                        format!("{}", code)
                    } else { "".into() }
                );
                env.insert(
                    "SIGNAL".into(),
                    if let Some(signal) = data.signal {
                        format!("{}", signal)
                    } else { "".into() }
                );
            },
            _ => unreachable!(),
        }

        env
    }
}


#[cfg(test)]
mod tests {
    use rustc_serialize::json;

    use utils::testing::*;
    use requests::RequestType;
    use jobs::JobOutput;
    use providers::Provider;

    use super::StatusProvider;


    fn dummy_job_output() -> JobOutput {
        JobOutput {
            stdout: "hello world".into(),
            stderr: "something went wrong".into(),

            success: true,
            exit_code: Some(0),
            signal: None,

            hook_name: "example".into(),
        }
    }


    #[test]
    fn config_hook_allowed() {
        macro_rules! assert_custom {
            ($hooks:expr, $check:expr, $expected:expr) => {{
                let provider = StatusProvider {
                    hooks: $hooks,
                    events: vec![],
                };
                assert_eq!(
                    provider.hook_allowed(&$check.to_string()),
                    $expected
                );
            }};
        };

        assert_custom!(None, "test", true);
        assert_custom!(Some(vec![]), "test", false);
        assert_custom!(Some(vec!["something".to_string()]), "test", false);
        assert_custom!(Some(vec!["test".to_string()]), "test", true);
    }


    #[test]
    fn config_event_allowed() {
        macro_rules! assert_custom {
            ($events:expr, $check:expr, $expected:expr) => {{
                let provider = StatusProvider {
                    hooks: None,
                    events: $events,
                };
                assert_eq!(
                    provider.event_allowed(&$check.to_string()),
                    $expected
                );
            }};
        };

        assert_custom!(vec![], "test", false);
        assert_custom!(vec!["something".to_string()], "test", false);
        assert_custom!(vec!["test".to_string()], "test", true);
    }


    #[test]
    fn test_new() {
        for right in &[
            r#"{"events": []}"#,
            r#"{"events": ["job_completed"]}"#,
            r#"{"events": ["job_completed", "job_failed"]}"#,
            r#"{"events": [], "hooks": []}"#,
            r#"{"events": [], "hooks": ["abc"]}"#,
        ] {
            assert!(StatusProvider::new(&right).is_ok());
        }

        for wrong in &[
            r#"{"hooks": 1}"#,
            r#"{"hooks": "a"}"#,
            r#"{"hooks": true}"#,
            r#"{"hooks": {}}"#,
            r#"{"hooks": [1]}"#,
            r#"{"hooks": [true]}"#,
            r#"{"hooks": []}"#,
            r#"{"hooks": ["abc"]}"#,
            r#"{"events": {}}"#,
            r#"{"events": [12345]}"#,
            r#"{"events": [true]}"#,
            r#"{"events": ["invalid_event"]}"#,
            r#"{"events": ["job_completed", "invalid_event"]}"#,
        ] {
            assert!(StatusProvider::new(&wrong).is_err());
        }
    }


    #[test]
    fn test_validate() {
        macro_rules! assert_validate {
            ($req:expr, $config:expr, $expect:expr) => {{
                let provider = StatusProvider::new($config).unwrap();
                assert_eq!(provider.validate($req), $expect)
            }};
        }
        let mut req = dummy_request();

        // Test without any of the required params
        assert_validate!(&req,
            r#"{"events": ["job_completed"]}"#,
            RequestType::Invalid
        );

        // Test without the right request body
        req.params.insert("event".into(), "job_completed".into());
        req.params.insert("hook_name".into(), "test".into());
        req.body = r#"{}"#.into();
        assert_validate!(&req,
            r#"{"events": ["job_completed"]}"#,
            RequestType::Invalid
        );

        // Test with the right request body for the event
        req.body = json::encode(&dummy_job_output()).unwrap();
        assert_validate!(&req,
            r#"{"events": ["job_completed"]}"#,
            RequestType::Internal
        );

        // Test with some extra params
        req.params.insert("test".into(), "invalid".into());
        assert_validate!(&req,
            r#"{"events": ["job_completed"]}"#,
            RequestType::Invalid
        );
        req.params.remove("test".into());

        // Test with a wrong allowed event
        assert_validate!(&req,
            r#"{"events": ["job_failed"]}"#,
            RequestType::Invalid
        );

        // Test with a right allowed event
        assert_validate!(&req,
            r#"{"events": ["job_completed"]}"#,
            RequestType::Internal
        );

        // Test with a wrong allowed hook
        assert_validate!(&req,
            r#"{"events": ["job_completed"], "hooks": ["invalid"]}"#,
            RequestType::Invalid
        );

        // Test with a right allowed hook
        assert_validate!(&req,
            r#"{"events": ["job_completed"], "hooks": ["test"]}"#,
            RequestType::Internal
        );

        // Test with a wrong event name
        req.params.insert("event".into(), "__invalid__".into());
        assert_validate!(&req,
            r#"{"events": ["job_completed"]}"#,
            RequestType::Invalid
        );
    }


    #[test]
    fn test_env() {
        let mut req = dummy_request();
        let provider = StatusProvider::new(
            r#"{"events": ["job_completed", "job_failed"]}"#
        ).unwrap();

        // Try with a job_completed event
        req.params.insert("event".into(), "job_completed".into());
        req.params.insert("hook_name".into(), "test".into());
        req.body = json::encode(&dummy_job_output()).unwrap();

        let env = provider.env(&req);
        assert_eq!(env.len(), 5);
        assert_eq!(env.get("EVENT").unwrap(), &"job_completed".to_string());
        assert_eq!(env.get("HOOK_NAME").unwrap(), &"test".to_string());
        assert_eq!(env.get("SUCCESS").unwrap(), &"1".to_string());
        assert_eq!(env.get("EXIT_CODE").unwrap(), &"0".to_string());
        assert_eq!(env.get("SIGNAL").unwrap(), &"".to_string());

        // Try with a job_failed event
        let mut output = dummy_job_output();
        output.success = false;
        output.exit_code = None;
        output.signal = Some(9);
        req.params.insert("event".into(), "job_failed".into());
        req.body = json::encode(&output).unwrap();

        let env = provider.env(&req);
        assert_eq!(env.len(), 5);
        assert_eq!(env.get("EVENT").unwrap(), &"job_failed".to_string());
        assert_eq!(env.get("HOOK_NAME").unwrap(), &"test".to_string());
        assert_eq!(env.get("SUCCESS").unwrap(), &"0".to_string());
        assert_eq!(env.get("EXIT_CODE").unwrap(), &"".to_string());
        assert_eq!(env.get("SIGNAL").unwrap(), &"9".to_string());
    }
}
