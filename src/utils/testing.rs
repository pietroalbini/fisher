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
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::fs;

use hyper::client as hyper;
use hyper::method::Method;

use common::prelude::*;
use common::state::State;
use common::structs::HealthDetails;
use common::config::{HttpConfig, RateLimitConfig};

use scripts::{Blueprint as HooksBlueprint, Repository as Hooks};
use scripts::{Job, JobOutput};
use web::{WebApp, WebRequest};
use utils;


#[macro_export]
macro_rules! assert_err {
    ($result:expr, $pattern:pat) => {{
        match $result {
            Ok(..) => {
                panic!("{} didn't error out",
                    stringify!($result)
                );
            },
            Err(error) => {
                match *error.kind() {
                    $pattern => {},
                    _ => {
                        panic!("{} didn't error with {}",
                            stringify!($result),
                            stringify!($pattern)
                        );
                    },
                }
            },
        }
    }};
}


#[macro_export]
macro_rules! hashmap {
    () => {{
        use std::collections::HashMap;
        HashMap::with_capacity(0)
    }};
    ($($key:expr => $val:expr,)*) => {{
        use std::collections::HashMap;

        let mut hm = HashMap::new();
        $( hm.insert($key, $val); )*
        hm
    }};
}


pub fn dummy_web_request() -> WebRequest {
    WebRequest {
        headers: HashMap::new(),
        params: HashMap::new(),
        source: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        body: String::new(),
    }
}


pub fn dummy_job_output() -> JobOutput {
    JobOutput {
        stdout: "hello world".into(),
        stderr: "something happened".into(),

        success: true,
        exit_code: Some(0),
        signal: None,

        script_name: "test".into(),
        request_ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),

        trigger_status_hooks: true,
    }
}


#[macro_export]
macro_rules! create_hook {
    ($tempdir:expr, $name:expr, $( $line:expr ),* ) => {{
        use std::fs;
        use std::os::unix::fs::OpenOptionsExt;
        use std::io::Write;

        let mut hook_path = $tempdir.clone();
        hook_path.push($name);

        let mut hook = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .mode(0o755)
            .open(&hook_path)
            .unwrap();

        let res = write!(hook, "{}", concat!(
            $(
                $line, "\n",
            )*
        ));
        res.unwrap();
    }};
}


pub fn sample_hooks() -> PathBuf {
    // Create a sample directory with some hooks
    let tempdir = utils::create_temp_dir().unwrap();

    create_hook!(
        tempdir,
        "example.sh",
        r#"#!/bin/bash"#,
        r#"## Fisher-Testing: {}"#,
        r#"echo "Hello world""#
    );

    create_hook!(
        tempdir,
        "failing.sh",
        r#"#!/bin/bash"#,
        r#"## Fisher-Testing: {}"#,
        r#"exit 1"#
    );

    create_hook!(
        tempdir,
        "jobs-details.sh",
        r#"#!/bin/bash"#,
        r#"## Fisher-Testing: {}"#,
        r#"b="${FISHER_TESTING_ENV}""#,
        r#"echo "executed" > "${b}/executed""#,
        r#"env > "${b}/env""#,
        r#"pwd > "${b}/pwd""#,
        r#"cat "${FISHER_REQUEST_BODY}" > "${b}/request_body""#,
        r#"cat "prepared" > "${b}/prepared""#
    );

    create_hook!(
        tempdir,
        "long.sh",
        r#"#!/bin/bash"#,
        r#"## Fisher-Testing: {}"#,
        r#"sleep 0.5"#,
        r#"echo "ok" > ${FISHER_TESTING_ENV}"#
    );

    create_hook!(
        tempdir,
        "append-val.sh",
        r#"#!/bin/bash"#,
        r#"## Fisher-Testing: {}"#,
        r#"data=(${FISHER_TESTING_ENV//>/ })"#,
        r#"echo "${data[1]}" >> "${data[0]}""#
    );

    create_hook!(
        tempdir,
        "trigger-status.sh",
        r#"#!/bin/bash"#,
        r#"## Fisher-Testing: {}"#,
        r#"echo "triggering...";"#
    );

    create_hook!(
        tempdir,
        "status-example.sh",
        r#"#!/bin/bash"#,
        concat!(
            r#"## Fisher-Status: {"events": ["job-completed", "job-failed"], "#,
            r#""scripts": ["trigger-status"]}"#,
        ),
        r#"echo "triggered!""#
    );

    fs::create_dir(&tempdir.join("sub")).unwrap();
    create_hook!(
        tempdir.join("sub"),
        "hook.sh",
        r#"#!/bin/bash"#,
        r#"## Fisher-Testing: {}"#,
        r#"echo "Hello world""#
    );

    tempdir
}


pub enum ProcessorApiCall {
    Queue(Job, isize),
    HealthDetails,
    Cleanup,
    Lock,
    Unlock,
}


pub struct FakeProcessorApi {
    sender: mpsc::Sender<ProcessorApiCall>,
}

impl ProcessorApiTrait<Hooks> for FakeProcessorApi {
    fn queue(&self, job: Job, priority: isize) -> Result<()> {
        self.sender.send(ProcessorApiCall::Queue(job, priority))?;
        Ok(())
    }

    fn health_details(&self) -> Result<HealthDetails> {
        self.sender.send(ProcessorApiCall::HealthDetails)?;
        Ok(HealthDetails {
            queued_jobs: 1,
            busy_threads: 2,
            max_threads: 3,
        })
    }

    fn cleanup(&self) -> Result<()> {
        self.sender.send(ProcessorApiCall::Cleanup)?;
        Ok(())
    }

    fn lock(&self) -> Result<()> {
        self.sender.send(ProcessorApiCall::Lock)?;
        Ok(())
    }

    fn unlock(&self) -> Result<()> {
        self.sender.send(ProcessorApiCall::Unlock)?;
        Ok(())
    }
}


pub struct WebAppInstance {
    inst: WebApp<FakeProcessorApi>,

    url: String,
    client: hyper::Client,

    processor_api_call: mpsc::Receiver<ProcessorApiCall>,
}

impl WebAppInstance {
    pub fn new(hooks: Arc<Hooks>, health: bool, behind_proxies: u8) -> Self {
        let (chan_send, chan_recv) = mpsc::channel();
        let fake_processor = FakeProcessorApi { sender: chan_send };

        // Start the web server
        // Create a new instance of WebApp
        let inst = WebApp::new(
            hooks,
            &HttpConfig {
                behind_proxies,
                bind: "127.0.0.1:0".parse().unwrap(),
                rate_limit: RateLimitConfig {
                    allowed: ::std::u64::MAX,
                    interval: ::std::u64::MAX.into(),
                },
                health_endpoint: health,
            },
            fake_processor,
        ).unwrap();

        // Create the HTTP client
        let url = format!("http://{}", inst.addr());
        let client = hyper::Client::new();

        WebAppInstance {
            inst: inst,

            url: url,
            client: client,
            processor_api_call: chan_recv,
        }
    }

    pub fn request(
        &mut self,
        method: Method,
        url: &str,
    ) -> hyper::RequestBuilder {
        // Create the HTTP request
        self.client.request(method, &format!("{}{}", self.url, url))
    }

    pub fn processor_input(&self) -> Option<ProcessorApiCall> {
        if let Ok(result) = self.processor_api_call.try_recv() {
            Some(result)
        } else {
            None
        }
    }

    pub fn lock(&self) {
        self.inst.lock();
    }

    pub fn unlock(&self) {
        self.inst.unlock();
    }

    pub fn stop(self) {
        self.inst.stop();
    }
}


pub struct TestingEnv {
    hooks: Arc<Hooks>,
    remove_dirs: Vec<String>,
}

impl TestingEnv {
    pub fn new() -> Self {
        let state = Arc::new(State::new());

        let hooks_dir = sample_hooks().to_str().unwrap().to_string();

        let mut hooks_blueprint = HooksBlueprint::new(state.clone());
        hooks_blueprint.collect_path(&hooks_dir, true).unwrap();

        TestingEnv {
            hooks: Arc::new(hooks_blueprint.repository()),
            remove_dirs: vec![hooks_dir],
        }
    }

    // CLEANUP

    pub fn cleanup(&self) {
        // Remove all the directories
        for dir in &self.remove_dirs {
            let _ = fs::remove_dir_all(dir);
        }
    }

    // WEB TESTING

    pub fn start_web(
        &self,
        health: bool,
        behind_proxies: u8,
    ) -> WebAppInstance {
        WebAppInstance::new(self.hooks.clone(), health, behind_proxies)
    }
}
