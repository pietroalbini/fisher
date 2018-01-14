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

use std::slice::Iter as SliceIter;
use std::net::IpAddr;

use serde_json;

use providers::prelude::*;
use scripts::JobOutput;


#[derive(Debug, Clone)]
pub enum StatusEvent {
    JobCompleted(JobOutput),
    JobFailed(JobOutput),
}

impl StatusEvent {
    #[inline]
    pub fn kind(&self) -> StatusEventKind {
        match *self {
            StatusEvent::JobCompleted(..) => StatusEventKind::JobCompleted,
            StatusEvent::JobFailed(..) => StatusEventKind::JobFailed,
        }
    }

    #[inline]
    pub fn script_name(&self) -> &str {
        match *self {
            StatusEvent::JobCompleted(ref output) |
            StatusEvent::JobFailed(ref output) => &output.script_name,
        }
    }

    #[inline]
    pub fn source_ip(&self) -> IpAddr {
        match *self {
            StatusEvent::JobCompleted(ref output) |
            StatusEvent::JobFailed(ref output) => output.request_ip,
        }
    }
}


#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StatusEventKind {
    JobCompleted,
    JobFailed,
}

impl StatusEventKind {
    fn name(&self) -> &str {
        match *self {
            StatusEventKind::JobCompleted => "job-completed",
            StatusEventKind::JobFailed => "job-failed",
        }
    }
}


#[derive(Debug, Deserialize)]
pub struct StatusProvider {
    events: Vec<StatusEventKind>,
    scripts: Option<Vec<String>>,
}

impl StatusProvider {
    #[inline]
    pub fn script_allowed(&self, name: &str) -> bool {
        // Check if it's allowed only if a whitelist was provided
        if let Some(ref scripts) = self.scripts {
            scripts.contains(&name.into())
        } else {
            true
        }
    }

    #[inline]
    pub fn events(&self) -> SliceIter<StatusEventKind> {
        self.events.iter()
    }
}

impl ProviderTrait for StatusProvider {
    fn new(config: &str) -> Result<Self> {
        Ok(serde_json::from_str(config)?)
    }

    fn validate(&self, request: &Request) -> RequestType {
        let req;
        if let Request::Status(ref inner) = *request {
            req = inner;
        } else {
            return RequestType::Invalid;
        }

        // The hook name must be allowed
        if !self.script_allowed(req.script_name()) {
            return RequestType::Invalid;
        }

        // The event must be allowed
        if !self.events.contains(&req.kind()) {
            return RequestType::Invalid;
        }

        RequestType::ExecuteHook
    }

    fn build_env(&self, req: &Request, b: &mut EnvBuilder) -> Result<()> {
        let req = if let Request::Status(ref inner) = *req {
            inner
        } else {
            return Ok(());
        };

        b.add_env("EVENT", req.kind().name());
        b.add_env("SCRIPT_NAME", req.script_name());

        match *req {
            StatusEvent::JobCompleted(ref out) => {
                b.add_env("SUCCESS", "1");
                b.add_env("EXIT_CODE", "0");
                b.add_env("SIGNAL", "");

                write!(b.data_file("stdout")?, "{}", out.stdout)?;
                write!(b.data_file("stderr")?, "{}", out.stderr)?;
            }
            StatusEvent::JobFailed(ref out) => {
                b.add_env("SUCCESS", "0");
                b.add_env("EXIT_CODE", if let Some(c) = out.exit_code {
                    c.to_string()
                } else {
                    String::with_capacity(0)
                });
                b.add_env("SIGNAL", if let Some(s) = out.signal {
                    s.to_string()
                } else {
                    String::with_capacity(0)
                });

                write!(b.data_file("stdout")?, "{}", out.stdout)?;
                write!(b.data_file("stderr")?, "{}", out.stderr)?;
            }
        }

        Ok(())
    }

    fn trigger_status_hooks(&self, _req: &Request) -> bool {
        // Don't trigger status hooks about status hooks
        // That would end really bad
        false
    }
}


#[cfg(test)]
mod tests {
    use utils::testing::*;
    use requests::RequestType;
    use providers::ProviderTrait;
    use scripts::EnvBuilder;

    use super::{StatusEvent, StatusProvider};


    #[test]
    fn config_script_allowed() {
        macro_rules! assert_custom {
            ($scripts:expr, $check:expr, $expected:expr) => {{
                let provider = StatusProvider {
                    scripts: $scripts,
                    events: vec![],
                };
                assert_eq!(
                    provider.script_allowed(&$check.to_string()),
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
    fn test_new() {
        for right in &[
            r#"{"events": []}"#,
            r#"{"events": ["job-completed"]}"#,
            r#"{"events": ["job-completed", "job-failed"]}"#,
            r#"{"events": [], "scripts": []}"#,
            r#"{"events": [], "scripts": ["abc"]}"#,
        ] {
            assert!(StatusProvider::new(&right).is_ok());
        }

        for wrong in &[
            r#"{"scripts": 1}"#,
            r#"{"scripts": "a"}"#,
            r#"{"scripts": true}"#,
            r#"{"scripts": {}}"#,
            r#"{"scripts": [1]}"#,
            r#"{"scripts": [true]}"#,
            r#"{"scripts": []}"#,
            r#"{"scripts": ["abc"]}"#,
            r#"{"events": {}}"#,
            r#"{"events": [12345]}"#,
            r#"{"events": [true]}"#,
            r#"{"events": ["invalid_event"]}"#,
            r#"{"events": ["job-completed", "invalid_event"]}"#,
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

        // Test with a wrong allowed event
        assert_validate!(
            &StatusEvent::JobCompleted(dummy_job_output()).into(),
            r#"{"events": ["job-failed"]}"#,
            RequestType::Invalid
        );

        // Test with a right allowed event
        assert_validate!(
            &StatusEvent::JobCompleted(dummy_job_output()).into(),
            r#"{"events": ["job-completed"]}"#,
            RequestType::ExecuteHook
        );

        // Test with a wrong allowed hook
        assert_validate!(
            &StatusEvent::JobCompleted(dummy_job_output()).into(),
            r#"{"events": ["job-completed"], "scripts": ["invalid"]}"#,
            RequestType::Invalid
        );

        // Test with a right allowed hook
        assert_validate!(
            &StatusEvent::JobCompleted(dummy_job_output()).into(),
            r#"{"events": ["job-completed"], "scripts": ["test"]}"#,
            RequestType::ExecuteHook
        );
    }


    #[test]
    fn test_env_builder_job_completed() {
        let provider = StatusProvider::new(
            r#"{"events": ["job-failed"]}"#,
        ).unwrap();

        let event = StatusEvent::JobCompleted(dummy_job_output());
        let mut b = EnvBuilder::dummy();
        provider.build_env(&event.into(), &mut b).unwrap();

        assert_eq!(b.dummy_data().env, hashmap! {
            "EVENT".into() => "job-completed".into(),
            "SCRIPT_NAME".into() => "test".into(),
            "SUCCESS".into() => "1".into(),
            "EXIT_CODE".into() => "0".into(),
            "SIGNAL".into() => "".into(),

            // File paths
            "STDOUT".into() => "stdout".into(),
            "STDERR".into() => "stderr".into(),
        });
        assert_eq!(b.dummy_data().files, hashmap! {
            "stdout".into() => "hello world".into(),
            "stderr".into() => "something happened".into(),
        });
    }


    #[test]
    fn test_env_builder_job_failed() {
        let provider = StatusProvider::new(
            r#"{"events": ["job-failed"]}"#,
        ).unwrap();

        let mut output = dummy_job_output();
        output.success = false;
        output.exit_code = None;
        output.signal = Some(9);

        let event = StatusEvent::JobFailed(output);
        let mut b = EnvBuilder::dummy();
        provider.build_env(&event.into(), &mut b).unwrap();

        assert_eq!(b.dummy_data().env, hashmap! {
            "EVENT".into() => "job-failed".into(),
            "SCRIPT_NAME".into() => "test".into(),
            "SUCCESS".into() => "0".into(),
            "EXIT_CODE".into() => "".into(),
            "SIGNAL".into() => "9".into(),

            // File paths
            "STDOUT".into() => "stdout".into(),
            "STDERR".into() => "stderr".into(),
        });
        assert_eq!(b.dummy_data().files, hashmap! {
            "stdout".into() => "hello world".into(),
            "stderr".into() => "something happened".into(),
        });
    }
}
