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
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::net::IpAddr;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;

use nix::unistd::{setpgid, Pid};
use tempdir::TempDir;
use users;

use common::prelude::*;
use common::state::UniqueId;

use scripts::Script;
use requests::Request;
use providers::Provider;


static DEFAULT_ENV: &[&'static str] = &[
    "PATH", "LC_ALL", "LANG",
];

static ENV_PREFIX: &'static str = "FISHER";


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


struct EnvBuilderReal<'job> {
    command: &'job mut Command,
    data_dir: &'job Path,
    last_file: Option<fs::File>,
}

#[cfg(test)]
pub struct EnvBuilderDummy {
    pub env: HashMap<String, String>,
    pub files: HashMap<String, Vec<u8>>,
}

enum EnvBuilderInner<'job> {
    Real(EnvBuilderReal<'job>),
    #[cfg(test)]
    Dummy(EnvBuilderDummy),
}

pub struct EnvBuilder<'job> {
    inner: EnvBuilderInner<'job>,
    prefix: Option<OsString>,
}

impl<'job> EnvBuilder<'job> {
    fn new(command: &'job mut Command, data_dir: &'job Path) -> Self {
        EnvBuilder {
            inner: EnvBuilderInner::Real(EnvBuilderReal {
                command,
                data_dir,
                last_file: None,
            }),
            prefix: Some(ENV_PREFIX.into()),
        }
    }

    #[cfg(test)]
    pub fn dummy() -> Self {
        EnvBuilder {
            inner: EnvBuilderInner::Dummy(EnvBuilderDummy {
                env: HashMap::new(),
                files: HashMap::new(),
            }),
            prefix: None,
        }
    }

    #[cfg(test)]
    pub fn dummy_data(&self) -> &EnvBuilderDummy {
        if let &EnvBuilderInner::Dummy(ref dummy) = &self.inner {
            dummy
        } else {
            panic!("called dummy_data on a non-dummy builder");
        }
    }

    fn set_prefix(&mut self, prefix: Option<&str>) {
        if let Some(prefix) = prefix {
            let prefix = prefix.chars()
                .map(|c| c.to_uppercase().to_string())
                .collect::<String>();

            self.prefix = Some(format!("{}_{}", ENV_PREFIX, prefix).into());
        } else {
            self.prefix = Some(ENV_PREFIX.into());
        }
    }

    fn env_name<N: AsRef<OsStr>>(&self, name: N) -> OsString {
        if let Some(ref prefix) = self.prefix {
            let mut result = prefix.clone();
            result.push("_");
            result.push(name);
            result
        } else {
            name.as_ref().into()
        }
    }

    fn clear_env(&mut self) {
        match self.inner {
            EnvBuilderInner::Real(ref mut inner) => {
                inner.command.env_clear();
            }
            #[cfg(test)]
            EnvBuilderInner::Dummy(ref mut inner) => {
                inner.env.clear();
            }
        }
    }

    fn add_env_unprefixed<K: AsRef<OsStr>, V: AsRef<OsStr>>(
        &mut self, k: K, v: V,
    ) {
        match self.inner {
            EnvBuilderInner::Real(ref mut inner) => {
                inner.command.env(k, v);
            }
            #[cfg(test)]
            EnvBuilderInner::Dummy(ref mut inner) => {
                inner.env.insert(
                    k.as_ref().to_str().unwrap().into(),
                    v.as_ref().to_str().unwrap().into(),
                );
            }
        }

    }

    pub fn add_env<K: AsRef<OsStr>, V: AsRef<OsStr>>(&mut self, k: K, v: V) {
        let name = self.env_name(k);
        self.add_env_unprefixed(name, v);
    }

    pub fn data_file<'a, P: AsRef<Path>>(
        &'a mut self, path: P,
    ) -> Result<&'a mut Write> {
        let env = path.as_ref().to_str().unwrap()
            .chars()
            .map(|c| c.to_uppercase().to_string())
            .collect::<String>();
        let name = self.env_name(env);

        match self.inner {
            EnvBuilderInner::Real(ref mut inner) => {
                let dest = inner.data_dir.join(&path);
                inner.command.env(name, &dest);

                inner.last_file = Some(fs::File::create(&dest)?);
                Ok(inner.last_file.as_mut().unwrap() as &mut Write)
            }
            #[cfg(test)]
            EnvBuilderInner::Dummy(ref mut inner) => {
                let dest = path.as_ref().to_str().unwrap().to_string();
                inner.env.insert(name.to_str().unwrap().into(), dest.clone());

                inner.files.insert(dest.clone(), Vec::new());
                Ok(inner.files.get_mut(&dest).unwrap() as &mut Write)
            }
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
        let mut command = Command::new(&self.script.exec());

        // Use random directories
        let working_directory = TempDir::new("fisher")?;

        // Prepare the command's environment
        {
            let mut builder = EnvBuilder::new(
                &mut command, &working_directory.path()
            );
            self.prepare_env(&mut builder, ctx)?;
        }

        command.current_dir(working_directory.path().to_str().unwrap());
        command.env("HOME", working_directory.path().to_str().unwrap());

        // Set the request IP
        command.env("FISHER_REQUEST_IP", self.request_ip().to_string());

        // Save the request body
        let request_body = self.save_request_body(working_directory.path())?;
        if let Some(path) = request_body {
            command.env("FISHER_REQUEST_BODY", path.to_str().unwrap());
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

        // The temp directory is dropped - and removed - here

        // Return the job output
        Ok(JobOutput::new(self, output))
    }

    fn prepare_env(
        &self, builder: &mut EnvBuilder, ctx: &Context,
    ) -> Result<()> {
        // First of all clear the environment
        builder.clear_env();

        // Set the USER environment variable with the correct username
        builder.add_env_unprefixed("USER", &ctx.username);

        // Apply the default environment
        // This is done (instead of the automatic inheritage) to whitelist
        // which environment variables we want
        for (key, value) in env::vars() {
            // Set only whitelisted keys
            if !DEFAULT_ENV.contains(&key.as_str()) {
                continue;
            }

            builder.add_env_unprefixed(key, value);
        }

        if let Some(ref provider) = self.provider {
            builder.set_prefix(Some(provider.name()));
            provider.build_env(&self.request, builder)?;
        }

        builder.set_prefix(None);

        Ok(())
    }

    fn save_request_body(&self, base: &Path) -> Result<Option<PathBuf>> {
        // Get the request body, even if some request kinds don't have one
        let body = match self.request {
            Request::Web(ref req) => &req.body,
            Request::Status(..) => return Ok(None),
        };

        let mut path = base.to_path_buf();
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
    fn new<'a>(job: &'a Job, output: Output) -> Self {
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
            r#"env"#,
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
        let result = job.process(ctx)?;

        if !result.success {
            println!("\nExit code: {:?}", result.exit_code);
            println!("Killed with signal: {:?}", result.signal);
            if result.stdout.trim().len() > 0 {
                println!("\nJob stdout:\n{}", result.stdout);
            }
            if result.stderr.trim().len() > 0 {
                println!("\nJob stderr:\n{}", result.stderr);
            }

            panic!("the job failed");
        }

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
                "FISHER_REQUEST_BODY", "FISHER_TESTING_PREPARED", "HOME",
                "USER",
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
