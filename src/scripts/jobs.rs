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

use nix::unistd::{setpgid, Pid};
use users;

use common::prelude::*;
use common::state::UniqueId;

use scripts::Script;
use utils;
use requests::Request;
use providers::Provider;


static DEFAULT_ENV: &[&'static str] = &[
    "PATH", "LC_ALL", "LANG",
];


#[derive(Debug)]
pub struct Context {
    pub environment: HashMap<String, String>,
    pub username: String,
}

impl Default for Context {
    fn default() -> Self {
        // Fallback to the user ID if the user is not in /etc/passwd
        let username = if let Some(name) = users::get_current_username() {
            name
        } else {
            users::get_current_uid().to_string()
        };

        Context {
            environment: HashMap::new(),
            username,
        }
    }
}


#[derive(Debug, Clone)]
pub struct Job {
    script: Arc<Script>,
    provider: Option<Arc<Provider>>,
    request: Request,
}

impl Job {
    pub fn new(
        script: Arc<Script>,
        provider: Option<Arc<Provider>>,
        request: Request,
    ) -> Job {
        Job {
            script,
            provider,
            request,
        }
    }

    pub fn request_ip(&self) -> IpAddr {
        match self.request {
            Request::Web(ref req) => req.source,
            Request::Status(ref req) => req.source_ip(),
        }
    }

    pub fn trigger_status_hooks(&self) -> bool {
        if let Some(ref provider) = self.provider {
            provider.trigger_status_hooks(&self.request)
        } else {
            true
        }
    }

    fn process(&self, ctx: &Context) -> Result<JobOutput> {
        let mut command = process::Command::new(&self.script.exec());

        // Prepare the command's environment variables
        self.prepare_env(&mut command, ctx);

        // Use a random working directory
        let working_directory = utils::create_temp_dir()?;
        command.current_dir(working_directory.to_str().unwrap());
        command.env("HOME", working_directory.to_str().unwrap());

        // Set the request IP
        command.env("FISHER_REQUEST_IP", self.request_ip().to_string());

        // Save the request body
        let request_body = self.save_request_body(&working_directory)?;
        if let Some(path) = request_body {
            command.env("FISHER_REQUEST_BODY", path.to_str().unwrap());
        }

        // Tell the provider to prepare the directory
        if let Some(ref provider) = self.provider {
            provider.prepare_directory(&self.request, &working_directory)?;
        }

        // Apply the custom environment
        for (key, value) in ctx.environment.iter() {
            command.env(&key, &value);
        }

        // Make sure the process is isolated
        command.before_exec(|| {
            // If a new process group is not created, the job still works fine
            let _ = setpgid(Pid::this(), Pid::from_raw(0));

            Ok(())
        });

        // Execute the hook
        let output = command.output()?;

        // Remove the temp directory
        fs::remove_dir_all(&working_directory)?;

        // Return the job output
        Ok(JobOutput::new(self, output))
    }

    fn prepare_env(&self, command: &mut process::Command, ctx: &Context) {
        // First of all clear the environment
        command.env_clear();

        // Set the USER environment variable with the correct username
        command.env("USER", ctx.username.clone());

        // Apply the default environment
        // This is done (instead of the automatic inheritage) to whitelist
        // which environment variables we want
        for (key, value) in env::vars() {
            // Set only whitelisted keys
            if !DEFAULT_ENV.contains(&key.as_str()) {
                continue;
            }

            command.env(key, value);
        }

        // Apply the hook-specific environment
        if let Some(ref provider) = self.provider {
            for (key, value) in provider.env(&self.request) {
                let real_key = format!(
                    "FISHER_{}_{}",
                    provider.name().to_uppercase(),
                    key
                );
                command.env(real_key, value);
            }
        }
    }

    fn save_request_body(&self, base: &PathBuf) -> Result<Option<PathBuf>> {
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

impl JobTrait<Script> for Job {
    type Context = Context;
    type Output = JobOutput;

    fn execute(&self, ctx: &Context) -> Result<JobOutput> {
        self.process(ctx)
    }

    fn script_id(&self) -> UniqueId {
        self.script.id()
    }

    fn script_name(&self) -> &str {
        self.script.name()
    }
}


#[derive(Debug, Clone)]
pub struct JobOutput {
    pub stdout: String,
    pub stderr: String,

    pub success: bool,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,

    pub script_name: String,
    pub request_ip: IpAddr,

    pub trigger_status_hooks: bool,
}

impl JobOutput {
    fn new<'a>(job: &'a Job, output: process::Output) -> Self {
        JobOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),

            success: output.status.success(),
            exit_code: output.status.code(),
            signal: output.status.signal(),

            script_name: job.script_name().into(),
            request_ip: job.request_ip(),

            trigger_status_hooks: job.trigger_status_hooks(),
        }
    }
}


#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::env;
    use std::ffi::OsString;
    use std::fs::File;
    use std::io::Read;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use users;

    use common::prelude::*;
    use requests::Request;
    use scripts::test_utils::*;
    use utils;

    use super::{Job, Context, DEFAULT_ENV};


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


    fn create_job(env: &TestEnv, name: &str, req: Request) -> Result<Job> {
        let script = env.load_script(name)?;
        let (_, provider) = script.validate(&req);

        Ok(Job::new(Arc::new(script), provider, req))
    }


    fn content<P: AsRef<Path>>(base: P, name: &str) -> Result<String> {
        let mut file = File::open(&base.as_ref().join(name))?;

        let mut buf = String::new();
        file.read_to_string(&mut buf)?;

        Ok(buf)
    }


    #[test]
    fn test_job_creation() {
        test_wrapper(|env| {
            // Create an example script
            env.create_script("example.sh", &[])?;

            // Create a new job from the script
            let req = dummy_web_request().into();
            let job = create_job(env, "example.sh", req)?;
            assert_eq!(job.script_name(), "example.sh");

            Ok(())
        });
    }


    #[test]
    fn test_job_execution() {
        test_wrapper(|env| {
            let ctx = Context::default();
            let req: Request = dummy_web_request().into();

            // Create a successful script
            env.create_script("success.sh", &[
                "#!/bin/bash",
                "exit 0",
            ])?;

            // Create a failing script
            env.create_script("fail.sh", &[
                "#!/bin/bash",
                "exit 1",
            ])?;

            // Execute the successful script
            let job = create_job(env, "success.sh", req.clone())?;
            let result = job.process(&ctx)?;
            assert!(result.success);
            assert_eq!(result.exit_code, Some(0));

            // Execute the failing script
            let job = create_job(env, "fail.sh", req.clone())?;
            let result = job.process(&ctx)?;
            assert!(!result.success);
            assert_eq!(result.exit_code, Some(1));

            Ok(())
        })
    }


    fn collect_env(env: &mut TestEnv, ctx: &Context) -> Result<PathBuf> {
        // Create a script that dumps the environment into files
        env.create_script("dump.sh", &[
            r#"#!/bin/bash"#,
            r#"## Fisher-Testing: {}"#,
            r#"b="${FISHER_TESTING_ENV}""#,
            r#"echo "executed" > "${b}/executed""#,
            r#"env > "${b}/env""#,
            r#"pwd > "${b}/pwd""#,
            r#"cat "${FISHER_REQUEST_BODY}" > "${b}/request_body""#,
        ])?;

        // Create a temp directory that contains the environment files
        let out = env.tempdir()?;

        // Create a dummy request
        let mut req = dummy_web_request();
        req.body = "a body!".into();
        req.params.insert("env".into(), out.to_str().unwrap().into());

        // Start the job
        let job = create_job(env, "dump.sh", req.into())?;
        job.process(ctx)?;

        Ok(out)
    }


    #[test]
    fn test_job_environment() {
        test_wrapper(|mut env| {
            let out = collect_env(&mut env, &Context::default())?;

            // Ensure the script was executed
            assert_eq!(&content(&out, "executed")?, "executed\n");

            // Ensure the request body was provided
            assert_eq!(&content(&out, "request_body")?, "a body!\n");

            // Get the script working directory
            let working_directory = content(&out, "pwd")?;

            // Parse the environment file
            let env_content = content(&out, "env")?;
            let env_vars = parse_env(&env_content);

            // Calculate the list of expected environment variables
            let extra_env = vec![
                // Variables set by Fisher
                "FISHER_TESTING_ENV", "FISHER_REQUEST_IP",
                "FISHER_REQUEST_BODY", "HOME", "USER",
                // Variables set by bash
                "PWD", "SHLVL", "_",
            ];
            let env_expected = DEFAULT_ENV.iter()
                .chain(extra_env.iter())
                .collect::<Vec<_>>();

            // Check if the right environment variables are present
            let mut found = 0;
            for (key, _) in &env_vars {
                if env_expected.contains(&key) {
                    found += 1;
                } else {
                    panic!("Extra environment variable found: {}", key);
                }
            }
            assert_eq!(found, env_expected.len());

            // Ensure environment variables are correct
            assert_eq!(&env_vars["FISHER_TESTING_ENV"], &out.to_str().unwrap());
            assert_eq!(&env_vars["FISHER_REQUEST_IP"], &"127.0.0.1");
            assert_eq!(&env_vars["HOME"], &working_directory.trim());
            assert_eq!(
                &env_vars["USER"],
                &users::get_current_username().unwrap()
            );
            for key in DEFAULT_ENV {
                if let Ok(content) = env::var(key) {
                    assert_eq!(&content, env_vars[key]);
                }
            }

            Ok(())
        });
    }


    #[test]
    fn test_job_environment_with_extra_env() {
        test_wrapper(|mut env| {
            // Create a custom context
            let ctx = Context {
                environment: {
                    let mut extra = HashMap::new();
                    extra.insert("TEST_ENV".into(), "yes".into());
                    extra
                },
                .. Context::default()
            };

            // Get the execution environment
            let out = collect_env(&mut env, &ctx)?;

            // Ensure the extra environment is present
            let env_content = content(&out, "env")?;
            let env_vars = parse_env(&env_content);
            assert_eq!(&env_vars["TEST_ENV"], &"yes");

            Ok(())
        });
    }


    #[test]
    fn test_job_environment_with_altered_user() {
        test_wrapper(|mut env| {
            // Execute a backup of the $USER environment variable
            let old_user = env::var_os("USER");

            // Change the $USER environment variable to a dummy value
            let mut new_name: OsString = if let Some(ref old) = old_user {
                old.into()
            } else {
                users::get_current_username().unwrap().into()
            };
            new_name.push("-dummy");
            env::set_var("USER", new_name);

            // Get the execution environment
            let out = collect_env(&mut env, &Context::default())?;

            // Ensure the $USER environment variable is correct
            let env_content = content(&out, "env")?;
            let env_vars = parse_env(&env_content);
            assert_eq!(
                &env_vars["USER"],
                &users::get_current_username().unwrap()
            );

            // Restore the $USER environment variable to its previous state
            if let Some(name) = old_user {
                env::set_var("USER", name);
            } else {
                env::remove_var("USER");
            }

            Ok(())
        });
    }
}
