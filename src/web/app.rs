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

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::net::SocketAddr;

use tiny_http::Method;

use common::prelude::*;

use scripts::Repository;
use web::http::HttpServer;
use web::api::WebApi;


pub struct WebApp<A: ProcessorApiTrait<Repository> + 'static> {
    server: HttpServer<WebApi<A>>,
    addr: SocketAddr,
    locked: Arc<AtomicBool>,
}

impl<A: ProcessorApiTrait<Repository>> WebApp<A> {
    pub fn new(
        hooks: Arc<Repository>,
        enable_health: bool,
        behind_proxies: u8,
        bind: &str,
        processor: A,
    ) -> Result<Self> {
        let locked = Arc::new(AtomicBool::new(false));

        // Create the web api
        let api = WebApi::new(processor, hooks, locked.clone(), enable_health);

        // Create the HTTP server
        let mut server = HttpServer::new(api, behind_proxies);
        server.add_route(Method::Get, "/health", Box::new(WebApi::get_health));
        server.add_route(
            Method::Get,
            "/hook/?",
            Box::new(WebApi::process_hook),
        );
        server.add_route(
            Method::Post,
            "/hook/?",
            Box::new(WebApi::process_hook),
        );

        let socket = server.listen(bind)?;

        Ok(WebApp {
            server: server,
            addr: socket,
            locked: locked,
        })
    }

    pub fn addr(&self) -> &SocketAddr {
        &self.addr
    }

    pub fn lock(&self) {
        self.locked.store(true, Ordering::SeqCst);
    }

    pub fn unlock(&self) {
        self.locked.store(false, Ordering::SeqCst);
    }

    pub fn stop(mut self) {
        self.server.stop();
    }
}


#[cfg(test)]
mod tests {
    use std::io::Read;

    use serde_json;
    use hyper::status::StatusCode;
    use hyper::method::Method;
    use hyper::header::Headers;

    use common::prelude::*;

    use utils::testing::*;


    #[test]
    fn test_startup() {
        let testing_env = TestingEnv::new();
        let mut inst = testing_env.start_web(true, 0);

        // Test if the Web API is working fine
        let res = inst.request(Method::Get, "/").send().unwrap();
        assert_eq!(res.status, StatusCode::NotFound);

        inst.stop();
        testing_env.cleanup();
    }

    #[test]
    fn test_hook_call() {
        let testing_env = TestingEnv::new();
        let mut inst = testing_env.start_web(true, 0);

        // It shouldn't be possible to call a non-existing hook
        let res = inst.request(Method::Get, "/hook/invalid.sh")
            .send()
            .unwrap();
        assert_eq!(res.status, StatusCode::NotFound);
        assert!(inst.processor_input().is_none());

        // Call the example hook without authorization
        let res = inst.request(Method::Get, "/hook/example.sh?secret=invalid")
            .send()
            .unwrap();
        assert_eq!(res.status, StatusCode::Forbidden);
        assert!(inst.processor_input().is_none());

        // Call the example hook with authorization
        let res = inst.request(Method::Get, "/hook/example.sh?secret=testing")
            .send()
            .unwrap();
        assert_eq!(res.status, StatusCode::Ok);

        // Assert a job is queued
        let input = inst.processor_input();

        // Assert the right job is queued
        if let ProcessorApiCall::Queue(job, _) = input.unwrap() {
            assert_eq!(job.script_name(), "example.sh");
        } else {
            panic!("Wrong processor input received");
        }

        // Call the example hook simulating a Ping
        let res =
            inst.request(Method::Get, "/hook/example.sh?request_type=ping")
                .send()
                .unwrap();
        assert_eq!(res.status, StatusCode::Ok);

        // Even if the last request succeded, there shouldn't be any job
        assert!(inst.processor_input().is_none());

        // Try to call an internal hook (in this case with the Status provider)
        let res = inst.request(
            Method::Get,
            concat!(
                "/hook/status-example.sh",
                "?event=job_completed",
                "&hook_name=trigger-status",
                "&exit_code=0",
                "&signal=0",
            ),
        ).send()
            .unwrap();
        assert_eq!(res.status, StatusCode::Forbidden);

        // Even if the last request succeded, there shouldn't be any job
        assert!(inst.processor_input().is_none());

        // Try to call an hook in a sub directory
        let res = inst.request(Method::Get, "/hook/sub/hook.sh")
            .send()
            .unwrap();
        assert_eq!(res.status, StatusCode::Ok);
        assert!(inst.processor_input().is_some());

        // Try on a locked instance
        inst.lock();

        // Even if this requets is valid, it should not be processed -- the
        // instance is locked
        let res = inst.request(Method::Get, "/hook/example.sh?secret=testing")
            .send()
            .unwrap();
        assert_eq!(res.status, StatusCode::ServiceUnavailable);
        assert!(inst.processor_input().is_none());

        // Now unlock the instance
        inst.unlock();

        // Call the example hook with authorization
        let res = inst.request(Method::Get, "/hook/example.sh?secret=testing")
            .send()
            .unwrap();
        assert_eq!(res.status, StatusCode::Ok);

        // Assert a job is queued
        assert!(inst.processor_input().is_some());

        inst.stop();
        testing_env.cleanup();
    }

    #[test]
    fn test_health_disabled() {
        // Create the instance with disabled health status
        let testing_env = TestingEnv::new();
        let mut inst = testing_env.start_web(false, 0);

        // It shouldn't be possible to get the health status
        let res = inst.request(Method::Get, "/health").send().unwrap();
        assert_eq!(res.status, StatusCode::Forbidden);

        inst.stop();
        testing_env.cleanup();
    }

    #[test]
    fn test_health_enabled() {
        // Create the instance with enabled health status
        let testing_env = TestingEnv::new();
        let mut inst = testing_env.start_web(true, 0);

        // Assert the request is OK
        let mut res = inst.request(Method::Get, "/health").send().unwrap();
        assert_eq!(res.status, StatusCode::Ok);

        // Decode the output
        let mut content = String::new();
        res.read_to_string(&mut content).unwrap();
        let data = serde_json::from_str::<serde_json::Value>(&content).unwrap();
        let data_obj = data.as_object().unwrap();

        // Check the content of the returned JSON
        let result = data_obj.get("result").unwrap().as_object().unwrap();
        assert_eq!(
            result.get("queued_jobs").unwrap().as_u64().unwrap(),
            1 as u64
        );
        assert_eq!(
            result.get("busy_threads").unwrap().as_u64().unwrap(),
            2 as u64
        );
        assert_eq!(
            result.get("max_threads").unwrap().as_u64().unwrap(),
            3 as u64
        );

        inst.stop();
        testing_env.cleanup();
    }

    #[test]
    fn test_behind_proxy() {
        // Create a new instance behind a proxy
        let testing_env = TestingEnv::new();
        let mut inst = testing_env.start_web(true, 1);

        // Call the example hook without a proxy
        let res = inst.request(Method::Get, "/hook/example.sh?ip=127.1.1.1")
            .send()
            .unwrap();
        assert_eq!(res.status, StatusCode::BadRequest);
        assert!(inst.processor_input().is_none());

        // Build the headers for a proxy
        let mut headers = Headers::new();
        headers.set_raw("X-Forwarded-For", vec![b"127.1.1.1".to_vec()]);

        // Make an example request
        let res = inst.request(Method::Get, "/hook/example.sh?ip=127.1.1.1")
            .headers(headers)
            .send()
            .unwrap();

        // The hook should be queued
        assert_eq!(res.status, StatusCode::Ok);
        assert!(inst.processor_input().is_some());

        inst.stop();
        testing_env.cleanup();
    }
}
