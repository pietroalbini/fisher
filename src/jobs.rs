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

use std::collections::HashMap;
use std::process;
use std::os::unix::process::ExitStatusExt;
use std::os::unix::process::CommandExt;
use std::fs;
use std::env;
use std::path::PathBuf;
use std::io::Write;
use std::sync::Arc;
use std::net::IpAddr;

use hooks::Hook;
use utils;
use requests::Request;
use providers::Provider;
use errors::FisherResult;
use native;


lazy_static! {
    static ref DEFAULT_ENV: Vec<String> = vec![
        "PATH".to_string(),
        "USER".to_string(),
        "SHELL".to_string(),

        // Internationalization stuff
        "LC_ALL".to_string(),
        "LANG".to_string(),
    ];
}


#[derive(Debug)]
pub struct Context {
    pub environment: HashMap<String, String>,
}

impl Default for Context {

    fn default() -> Self {
        Context {
            environment: HashMap::new(),
        }
    }
}


#[derive(Debug, Clone)]
pub struct Job {
    hook: Arc<Hook>,
    provider: Option<Arc<Provider>>,
    request: Request,
}

impl Job {

    pub fn new(hook: Arc<Hook>, provider: Option<Arc<Provider>>,
               request: Request) -> Job {
        Job {
            hook: hook,
            provider: provider,
            request: request,
        }
    }

    #[inline]
    pub fn hook_name(&self) -> &str {
        self.hook.name()
    }

    #[inline]
    pub fn request_ip(&self) -> IpAddr {
        match self.request {
            Request::Web(ref req) => req.source,
            Request::Status(ref req) => req.source_ip(),
        }
    }

    pub fn process(&self, ctx: &Context) -> FisherResult<JobOutput> {
        let mut command = process::Command::new(&self.hook.exec());

        // Prepare the command's environment variables
        self.prepare_env(&mut command);

        // Use a random working directory
        let working_directory = utils::create_temp_dir()?;
        command.current_dir(working_directory.to_str().unwrap());
        command.env("HOME".to_string(), working_directory.to_str().unwrap());

        // Set the request IP
        command.env(
            "FISHER_REQUEST_IP".to_string(),
            format!("{}", self.request_ip())
        );

        // Save the request body
        let request_body = self.save_request_body(&working_directory)?;
        if let Some(path) = request_body {
            command.env(
                "FISHER_REQUEST_BODY".to_string(),
                path.to_str().unwrap().to_string()
            );
        }

        // Tell the provider to prepare the directory
        if let Some(ref provider) = self.provider {
            provider.prepare_directory(
                &self.request, &working_directory
            )?;
        }

        // Apply the custom environment
        for (key, value) in ctx.environment.iter() {
            command.env(&key, &value);
        }

        // Make sure the process is isolated
        command.before_exec(|| {
            native::isolate_process();
            Ok(())
        });

        // Execute the hook
        let output = command.output()?;

        // Remove the temp directory
        fs::remove_dir_all(&working_directory)?;

        // Return the job output
        Ok((self, output).into())
    }

    fn prepare_env(&self, command: &mut process::Command) {
        // First of all clear the environment
        command.env_clear();

        // Apply the default environment
        // This is done (instead of the automatic inheritage) to whitelist
        // which environment variables we want
        for (key, value) in env::vars() {
            // Set only whitelisted keys
            if ! DEFAULT_ENV.contains(&key) {
                continue;
            }

            command.env(key, value);
        }

        // Apply the hook-specific environment
        if let Some(ref provider) = self.provider {
            for (key, value) in provider.env(&self.request) {
                let real_key = format!(
                    "FISHER_{}_{}", provider.name().to_uppercase(), key
                );
                command.env(real_key, value);
            }
        }
    }

    fn save_request_body(&self, base: &PathBuf)
                        -> FisherResult<Option<PathBuf>> {
        // Get the request body, even if some request kinds don't have one
        let body = match self.request {
            Request::Web(ref req) => &req.body,
            Request::Status(..) => return Ok(None),
        };

        let mut path = base.clone();
        path.push("request_body");

        // Write the request body on disk
        let mut file = fs::File::create(&path)?;
        write!(file, "{}\n", body)?;

        Ok(Some(path))
    }
}


#[derive(Debug, Clone)]
pub struct JobOutput {
    pub stdout: String,
    pub stderr: String,

    pub success: bool,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,

    pub hook_name: String,
    pub request_ip: IpAddr,
}

impl<'a> From<(&'a Job, process::Output)> for JobOutput {

    fn from(data: (&'a Job, process::Output)) -> JobOutput {
        JobOutput {
            stdout: String::from_utf8_lossy(&data.1.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&data.1.stderr).into_owned(),

            success: data.1.status.success(),
            exit_code: data.1.status.code(),
            signal: data.1.status.signal(),

            hook_name: data.0.hook_name().into(),
            request_ip: data.0.request_ip(),
        }
    }
}


#[cfg(test)]
mod tests {
    use std::env;
    use std::collections::HashMap;

    use utils::testing::*;
    use utils;

    use super::{DEFAULT_ENV, Context};


    macro_rules! read {
        ($output:expr, $name:expr) => {{
            use std::fs;
            use std::io::Read;

            let file_name = format!("{}/{}", $output, $name);
            let mut file = fs::File::open(&file_name).unwrap();

            let mut buf = String::new();
            file.read_to_string(&mut buf).unwrap();

            buf
        }};
    }


    fn parse_env(content: &str) -> HashMap<&str, &str> {
        let mut result = HashMap::new();

        for line in content.split("\n") {
            // Skip empty lines
            if line.trim() == "" {
                continue;
            }

            let (key, value) = utils::parse_env(line).unwrap();
            result.insert(key, value);
        }

        result
    }


    #[test]
    fn test_job_creation() {
        let env = TestingEnv::new();

        let _ = env.create_job("example.sh", dummy_web_request().into());

        env.cleanup();
    }


    #[test]
    fn test_job_hook_name() {
        let env = TestingEnv::new();

        let job = env.create_job("example.sh", dummy_web_request().into());
        assert_eq!(job.hook_name(), "example.sh".to_string());

        env.cleanup();
    }

    #[test]
    fn test_job_execution() {
        let env = TestingEnv::new();
        let ctx = Context::default();

        // The "example" hook should be processed without problems
        let job = env.create_job("example.sh", dummy_web_request().into());
        let result = job.process(&ctx).unwrap();
        assert!(result.success);
        assert_eq!(result.exit_code, Some(0));

        let job = env.create_job("failing.sh", dummy_web_request().into());
        let result = job.process(&ctx).unwrap();
        assert!(! result.success);
        assert_eq!(result.exit_code, Some(1));

        env.cleanup();
    }

    #[test]
    fn test_job_environment() {
        let mut env = TestingEnv::new();
        let ctx = Context::default();

        // Create a temp directory which will contain the build
        let output_path = utils::create_temp_dir().unwrap();
        let output = output_path.to_str().unwrap();
        env.delete_also(&output);

        // Create a new dummy request
        let mut req = dummy_web_request();
        req.body = "a body!".to_string();
        req.params.insert("env".to_string(), output.to_string());

        // Process the job
        let job = env.create_job("jobs-details.sh", req.into());
        assert!(job.process(&ctx).is_ok());

        // The hook must be executed
        assert_eq!(read!(output, "executed"), "executed\n".to_string());

        // The request body must be present
        assert_eq!(read!(output, "request_body"), "a body!\n".to_string());

        // The file from prepare_directory must be present
        assert_eq!(read!(output, "prepared"), "prepared\n".to_string());

        // Get the used working directory
        let pwd_raw = read!(output, "pwd");
        let working_directory = pwd_raw.trim();

        // Parse the environment file
        let raw_env = read!(output, "env");
        let job_env = parse_env(&raw_env);

        // Get all the required environment variables
        let mut required_env = {
            let mut res: Vec<&str> = DEFAULT_ENV.iter().map(|i| {
                i.as_str()
            }).collect();

            // Those are from the provider
            res.push("FISHER_TESTING_ENV");

            // Those are added by the processor
            res.push("HOME");
            res.push("FISHER_REQUEST_BODY");
            res.push("FISHER_REQUEST_IP");

            // Those are extra variables added by bash
            res.push("PWD");
            res.push("SHLVL");
            res.push("_");

            res
        };

        // Check if the right environment variables are present
        let mut found = vec![];
        for (key, _) in &job_env {
            if required_env.contains(key) {
                found.push(key);
            } else {
                panic!("Extra env variable: {}", key);
            }
        }
        assert_eq!(required_env.sort(), found.sort());

        // The env var generated from the provider must be present
        assert_eq!(
            *job_env.get("FISHER_TESTING_ENV").unwrap(),
            output.to_string()
        );

        // $HOME must be the current directory
        assert_eq!(
            *job_env.get("HOME").unwrap(),
            working_directory
        );

        // The IP address must be correct
        // dummy_web_request() sets it to 127.0.0.1
        assert_eq!(
            *&job_env.get("FISHER_REQUEST_IP").unwrap(),
            &"127.0.0.1"
        );

        // The value of the environment variables forwarded from the current
        // env must have the same content of the current env
        for key in DEFAULT_ENV.iter() {
            // If the key is not present in the testing environment, ignore it
            match env::var(key) {
                Ok(content) => {
                    assert_eq!(
                        content.as_str(),
                        *job_env.get(key.as_str()).unwrap()
                    );
                },
                Err(..) => {},
            }
        }

        env.cleanup();
    }


    #[test]
    fn test_environment_with_context() {
        let mut env = TestingEnv::new();

        // Add an extra environment variable to the context
        let ctx = Context {
            environment: {
                let mut extra_env = HashMap::new();
                extra_env.insert("TEST_ENV".into(), "yes".into());
                extra_env
            },
        };

        // Create a temp directory which will contain the output
        let output_path = utils::create_temp_dir().unwrap();
        let output = output_path.to_str().unwrap();
        env.delete_also(&output);

        // Create a dummy web request
        let mut req = dummy_web_request();
        req.params.insert("env".into(), output.to_string());

        // Process the job
        let job = env.create_job("jobs-details.sh", req.into());
        assert!(job.process(&ctx).is_ok());

        let raw_env = read!(output, "env");
        let job_env = parse_env(&raw_env);

        // Test that the extra environment variable is present
        assert_eq!(*job_env.get("TEST_ENV").unwrap(), "yes");

        env.cleanup();
    }
}
