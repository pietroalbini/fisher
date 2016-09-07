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

use std::process;
use std::os::unix::process::ExitStatusExt;
use std::fs;
use std::env;
use std::path::PathBuf;
use std::io::Write;

use hooks::JobHook;
use utils;
use web::requests::Request;
use errors::{FisherError, ErrorKind, FisherResult};


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


#[derive(Clone)]
pub struct Job {
    hook: JobHook,
    request: Request,
}

impl Job {

    pub fn new(hook: JobHook, request: Request) -> Job {
        Job {
            hook: hook,
            request: request,
        }
    }

    pub fn hook_name(&self) -> String {
        self.hook.name()
    }

    pub fn process(&self) -> FisherResult<()> {
        let mut command = process::Command::new(self.hook.exec());

        // Prepare the command's environment variables
        self.prepare_env(&mut command);

        // Use a random working directory
        let working_directory = try!(utils::create_temp_dir());
        command.current_dir(working_directory.to_str().unwrap());
        command.env("HOME".to_string(), working_directory.to_str().unwrap());

        // Save the request body
        let request_body = try!(self.save_request_body(&working_directory));
        command.env(
            "FISHER_REQUEST_BODY".to_string(),
            request_body.to_str().unwrap().to_string()
        );

        // Execute the hook
        let output = try!(command.output());
        if ! output.status.success() {
            return Err(FisherError::new(ErrorKind::HookExecutionFailed(
                output.status.code(),
                output.status.signal(),
            )));
        }

        // Remove the temp directory
        try!(fs::remove_dir_all(&working_directory));

        Ok(())
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
        if let Some(ref provider_name) = self.hook.provider_name() {
            for (key, value) in self.hook.env(&self.request) {
                let real_key = format!(
                    "FISHER_{}_{}", provider_name.to_uppercase(), key
                );
                command.env(real_key, value);
            }
        }
    }

    fn save_request_body(&self, base: &PathBuf) -> FisherResult<PathBuf> {
        let mut path = base.clone();
        path.push("request_body");

        // Write the request body on disk
        let mut file = try!(fs::File::create(&path));
        try!(write!(file, "{}\n", self.request.body));

        Ok(path)
    }
}


#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;

    use hooks;
    use web::requests;

    use utils::testing::*;

    use super::Job;


    struct TestEnv {
        hooks_dir: String,
        hooks: HashMap<String, hooks::Hook>,
    }

    impl TestEnv {

        fn new() -> Self {
            let hooks_dir = sample_hooks().to_str().unwrap().to_string();
            let hooks = hooks::collect(&hooks_dir).unwrap();

            TestEnv {
                hooks_dir: hooks_dir,
                hooks: hooks,
            }
        }

        fn create_job(&self, hook_name: &str, req: requests::Request) -> Job {
            // Get the JobHook
            let hook = self.hooks.get(&hook_name.to_string()).unwrap();
            let job_hook = hook.validate(&req).unwrap();

            Job::new(job_hook, req)
        }

        fn cleanup(&self) {
            let _ = fs::remove_dir_all(&self.hooks_dir);
        }
    }


    #[test]
    fn test_job_creation() {
        let env = TestEnv::new();

        let _ = env.create_job("example", dummy_request());

        env.cleanup();
    }


    #[test]
    fn test_job_hook_name() {
        let env = TestEnv::new();

        let job = env.create_job("example", dummy_request());
        assert_eq!(job.hook_name(), "example".to_string());

        env.cleanup();
    }
}
