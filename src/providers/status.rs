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

use std::fs;
use std::io::Write;
use std::slice::Iter as SliceIter;
use std::net::IpAddr;

use serde_json;

use providers::prelude::*;
use jobs::JobOutput;


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
    pub fn hook_name(&self) -> &String {
        match *self {
            StatusEvent::JobCompleted(ref output) |
            StatusEvent::JobFailed(ref output) => &output.hook_name,
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
pub enum StatusEventKind {
    #[serde(rename = "job_completed")]
    JobCompleted,
    #[serde(rename = "job_failed")]
    JobFailed,
}

impl StatusEventKind {

    fn name(&self) -> &str {
        match *self {
            StatusEventKind::JobCompleted => "job_completed",
            StatusEventKind::JobFailed => "job_failed",
        }
    }
}


#[derive(Debug, Deserialize)]
pub struct StatusProvider {
    events: Vec<StatusEventKind>,
    hooks: Option<Vec<String>>,
}

impl StatusProvider {

    #[inline]
    pub fn hook_allowed(&self, name: &str) -> bool {
        // Check if it's allowed only if a whitelist was provided
        if let Some(ref hooks) = self.hooks {
            if ! hooks.contains(&name.into()) {
                return false;
            }
        }

        true
    }

    #[inline]
    pub fn events(&self) -> SliceIter<StatusEventKind> {
        self.events.iter()
    }
}

impl ProviderTrait for StatusProvider {

    fn new(config: &str) -> FisherResult<Self> {
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
        if ! self.hook_allowed(req.hook_name()) {
            return RequestType::Invalid;
        }

        // The event must be allowed
        if ! self.events.contains(&req.kind()) {
            return RequestType::Invalid;
        }

        RequestType::ExecuteHook
    }

    fn env(&self, request: &Request) -> HashMap<String, String> {
        let mut env = HashMap::new();

        let req;
        if let Request::Status(ref inner) = *request {
            req = inner;
        } else {
            return env;
        }

        env.insert("EVENT".into(), req.kind().name().into());
        env.insert("HOOK_NAME".into(), req.hook_name().clone());

        // Event-specific env
        match *req {
            StatusEvent::JobCompleted(..) => {
                env.insert("SUCCESS".into(), "1".into());
                env.insert("EXIT_CODE".into(), "0".into());
                env.insert("SIGNAL".into(), String::new());
            },
            StatusEvent::JobFailed(ref output) => {
                env.insert("SUCCESS".into(), "0".into());
                env.insert(
                    "EXIT_CODE".into(),
                    if let Some(code) = output.exit_code {
                        format!("{}", code)
                    } else { String::new() }
                );
                env.insert(
                    "SIGNAL".into(),
                    if let Some(signal) = output.signal {
                        format!("{}", signal)
                    } else { String::new() }
                );
            },
        }

        env
    }

    fn prepare_directory(&self, req: &Request, path: &PathBuf)
                         -> FisherResult<()> {
        let req = req.status()?;

        macro_rules! new_file {
            ($base:expr, $name:expr, $content:expr) => {{
                let mut path = $base.clone();
                path.push($name);

                let mut file = fs::File::create(&path)?;
                write!(file, "{}", $content)?;
            }};
        }

        match *req {
            StatusEvent::JobCompleted(ref output) => {
                new_file!(path, "stdout", output.stdout);
                new_file!(path, "stderr", output.stderr);
            },
            StatusEvent::JobFailed(ref output) => {
                new_file!(path, "stdout", output.stdout);
                new_file!(path, "stderr", output.stderr);
            },
        }

        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use std::fs;

    use utils::testing::*;
    use utils;
    use requests::RequestType;
    use providers::ProviderTrait;

    use super::{StatusEvent, StatusProvider};


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

        // Test with a wrong allowed event
        assert_validate!(&StatusEvent::JobCompleted(dummy_job_output()).into(),
            r#"{"events": ["job_failed"]}"#,
            RequestType::Invalid
        );

        // Test with a right allowed event
        assert_validate!(&StatusEvent::JobCompleted(dummy_job_output()).into(),
            r#"{"events": ["job_completed"]}"#,
            RequestType::ExecuteHook
        );

        // Test with a wrong allowed hook
        assert_validate!(&StatusEvent::JobCompleted(dummy_job_output()).into(),
            r#"{"events": ["job_completed"], "hooks": ["invalid"]}"#,
            RequestType::Invalid
        );

        // Test with a right allowed hook
        assert_validate!(&StatusEvent::JobCompleted(dummy_job_output()).into(),
            r#"{"events": ["job_completed"], "hooks": ["test"]}"#,
            RequestType::ExecuteHook
        );
    }


    #[test]
    fn test_env() {
        let provider = StatusProvider::new(
            r#"{"events": ["job_completed", "job_failed"]}"#
        ).unwrap();

        // Try with a job_completed event
        let event = StatusEvent::JobCompleted(dummy_job_output());
        let env = provider.env(&event.into());
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

        let env = provider.env(&StatusEvent::JobFailed(output).into());
        assert_eq!(env.len(), 5);
        assert_eq!(env.get("EVENT").unwrap(), &"job_failed".to_string());
        assert_eq!(env.get("HOOK_NAME").unwrap(), &"test".to_string());
        assert_eq!(env.get("SUCCESS").unwrap(), &"0".to_string());
        assert_eq!(env.get("EXIT_CODE").unwrap(), &"".to_string());
        assert_eq!(env.get("SIGNAL").unwrap(), &"9".to_string());
    }

    #[test]
    fn test_prepare_directory() {
        macro_rules! read {
            ($base:expr, $name:expr) => {{
                use std::fs;
                use std::io::Read;

                let base = $base.as_path().to_str().unwrap().to_string();
                let file_name = format!("{}/{}", base, $name);
                let mut file = fs::File::open(&file_name).unwrap();

                let mut buf = String::new();
                file.read_to_string(&mut buf).unwrap();

                buf
            }};
        }

        let provider = StatusProvider::new(
            r#"{"events": ["job_completed"]}"#,
        ).unwrap();

        let event = StatusEvent::JobCompleted(dummy_job_output());
        let tempdir = utils::create_temp_dir().unwrap();
        provider.prepare_directory(&event.into(), &tempdir).unwrap();

        assert_eq!(read!(tempdir, "stdout"), "hello world".to_string());
        assert_eq!(read!(tempdir, "stderr"), "something happened".to_string());

        fs::remove_dir_all(&tempdir).unwrap();
    }
}
