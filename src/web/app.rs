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

use std::net::SocketAddr;
use std::sync::Arc;

use chan;
use nickel::{Nickel, HttpRouter, Options};
use hyper::header;
use rustc_serialize::json::ToJson;

use app::FisherOptions;
use hooks::Hooks;
use processor::SenderChan;
use web::responses::Response as FisherResponse;
use web::proxies::ProxySupport;
use requests::convert_request;
use web::api::WebApi;


macro_rules! handler {
    ( $app:expr, $handler:path $(, $param:expr )* ) => {{
        let web_api = $app.web_api.clone().unwrap();
        let proxy_support = $app.proxy_support.clone().unwrap();

        middleware! { |req, mut res|
            // Make the req object mutable
            let mut req = req;

            let response;
            let mut request = convert_request(&mut req);

            // Call the handler if the request is OK, else return a
            // BadRequest response
            if let Err(error) = proxy_support.fix_request(&mut request) {
                response = FisherResponse::BadRequest(error);
            } else {
                response = $handler(
                    &*web_api, request,
                    $(
                        req.param($param).unwrap().into()
                    )*
                );
            }

            res.set(response.status());
            response.to_json()
        }
    }};
}


pub struct WebApp {
    stop_chan: Option<chan::Sender<()>>,
    sender_chan: Option<SenderChan>,
    stop: bool,
    hooks: Arc<Hooks>,

    proxy_support: Option<Arc<ProxySupport>>,
    web_api: Option<Arc<WebApi>>,
}

impl WebApp {

    pub fn new(hooks: Arc<Hooks>) -> Self {
        WebApp {
            stop_chan: None,
            sender_chan: None,
            stop: false,
            hooks: hooks,

            proxy_support: None,
            web_api: None,
        }
    }

    pub fn listen(&mut self, options: &FisherOptions, sender: SenderChan)
                  -> Result<SocketAddr, String> {
        // Store the sender channel
        self.sender_chan = Some(sender);

        // This is to fix lifetime issues with the thread below
        let bind = options.bind.to_string();

        // Configure the proxy support
        self.proxy_support = Some(Arc::new(
            ProxySupport::new(&options)
        ));

        // Create a new instance of the API
        self.web_api = Some(Arc::new(WebApi::new(
            self.sender_chan.clone().unwrap(),
            self.hooks.clone(),
            options.enable_health,
        )));

        let app = self.configure_nickel();

        // This channel is used so it's possible to stop the listener
        let (send_stop, recv_stop) = chan::sync(0);
        self.stop_chan = Some(send_stop);

        // This channel is used to receive the result from the thread, which
        // will be returned
        let (return_send, return_recv) = chan::async();

        ::std::thread::spawn(move || {
            let bind: &str = &bind;
            match app.listen(bind) {
                Ok(listener) => {
                    // Send the socket address to the main thread
                    let sock = listener.socket();
                    return_send.send(Ok(sock));

                    // This blocks until someone sends something to
                    // self.stop_chan
                    recv_stop.recv();

                    // Detach the webserver from the current thread, allowing
                    // the process to exit
                    listener.detach();
                },
                Err(error) => {
                    // Send the error
                    return_send.send(Err(format!("{}", error)));
                }
            }
        });

        // Return what the thread sends
        return_recv.recv().unwrap()
    }

    pub fn stop(&mut self) -> bool {
        // Don't try to stop twice
        if self.stop {
            return true;
        }

        match self.stop_chan {
            Some(ref stop_chan) => {
                // Tell the thread to stop
                stop_chan.send(());

                self.stop = true;
                true
            },
            None => false,
        }
    }

    fn configure_nickel(&self) -> Nickel {
        let mut app = Nickel::new();

        // Disable the default message nickel prints on stdout
        app.options = Options::default().output_on_listen(false);

        app.utilize(middleware! { |_req, mut res|
            res.set(header::Server(
                format!("Fisher/{}", crate_version!())
            ));
        });

        app.get("/hook/:hook", handler!(self, WebApi::process_hook, "hook"));
        app.post("/hook/:hook", handler!(self, WebApi::process_hook, "hook"));

        app.get("/health", handler!(self, WebApi::get_health));

        // This is the not found handler
        app.utilize(handler!(self, WebApi::not_found));

        app
    }

}


#[cfg(test)]
mod tests {
    use std::io::Read;

    use hyper::status::StatusCode;
    use hyper::method::Method;
    use hyper::header::Headers;
    use rustc_serialize::json::Json;

    use utils::testing::*;
    use processor::{HealthDetails, ProcessorInput};


    #[test]
    fn test_startup() {
        let testing_env = TestingEnv::new();
        let mut inst = testing_env.start_web(true, None);

        // Test if the Web API is working fine
        let res = inst.request(Method::Get, "/").send().unwrap();
        assert_eq!(res.status, StatusCode::NotFound);

        inst.stop();
        testing_env.cleanup();
    }

    #[test]
    fn test_hook_call() {
        let testing_env = TestingEnv::new();
        let mut inst = testing_env.start_web(true, None);

        // It shouldn't be possible to call a non-existing hook
        let res = inst.request(Method::Get, "/hook/invalid")
                      .send().unwrap();
        assert_eq!(res.status, StatusCode::NotFound);
        assert!(inst.processor_input().is_none());

        // Call the example hook without authorization
        let res = inst.request(Method::Get, "/hook/example?secret=invalid")
                      .send().unwrap();
        assert_eq!(res.status, StatusCode::Forbidden);
        assert!(inst.processor_input().is_none());

        // Call the example hook with authorization
        let res = inst.request(Method::Get, "/hook/example?secret=testing")
                      .send().unwrap();
        assert_eq!(res.status, StatusCode::Ok);

        // Assert a job is queued
        let input = inst.processor_input();
        assert!(input.is_some());

        // Assert the right job is queued
        if let ProcessorInput::Job(job) = input.unwrap() {
            assert_eq!(job.hook_name(), "example");
        } else {
            panic!("Wrong processor input received");
        }

        // Call the example hook simulating a Ping
        let res = inst.request(Method::Get, "/hook/example?request_type=ping")
                      .send().unwrap();
        assert_eq!(res.status, StatusCode::Ok);

        // Even if the last request succeded, there shouldn't be any job
        assert!(inst.processor_input().is_none());

        // Try to call an internal hook (in this case with the Status provider)
        let res = inst.request(Method::Get, concat!(
            "/hook/status-example",
            "?event=job_completed",
            "&hook_name=trigger-status",
            "&exit_code=0",
            "&signal=0",
        )).send().unwrap();
        assert_eq!(res.status, StatusCode::Forbidden);

        // Even if the last request succeded, there shouldn't be any job
        assert!(inst.processor_input().is_none());

        inst.stop();
        testing_env.cleanup();
    }

    #[test]
    fn test_health_disabled() {
        // Create the instance with disabled health status
        let testing_env = TestingEnv::new();
        let mut inst = testing_env.start_web(false, None);

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
        let mut inst = testing_env.start_web(true, None);

        let check_after = inst.next_health(HealthDetails {
            queue_size: 1,
            active_jobs: 2,
        });

        // Assert the request is OK
        let mut res = inst.request(Method::Get, "/health").send().unwrap();
        assert_eq!(res.status, StatusCode::Ok);

        // Decode the output
        let mut content = String::new();
        res.read_to_string(&mut content).unwrap();
        let data = Json::from_str(&content).unwrap();
        let data_obj = data.as_object().unwrap();

        // Check the content of the returned JSON
        let result = data_obj.get("result").unwrap().as_object().unwrap();
        assert_eq!(
            result.get("queue_size").unwrap().as_u64().unwrap(),
            1 as u64
        );
        assert_eq!(
            result.get("active_jobs").unwrap().as_u64().unwrap(),
            2 as u64
        );

        // Check if there were any problems into the next_health thread
        check_after.check();

        inst.stop();
        testing_env.cleanup();
    }

    #[test]
    fn test_behind_proxy() {
        // Create a new instance behind a proxy
        let testing_env = TestingEnv::new();
        let mut inst = testing_env.start_web(true, Some(1));

        // Call the example hook without a proxy
        let res = inst.request(Method::Get, "/hook/example?ip=127.1.1.1")
                      .send().unwrap();
        assert_eq!(res.status, StatusCode::BadRequest);
        assert!(inst.processor_input().is_none());

        // Build the headers for a proxy
        let mut headers = Headers::new();
        headers.set_raw("X-Forwarded-For", vec![b"127.1.1.1".to_vec()]);

        // Make an example request
        let res = inst.request(Method::Get, "/hook/example?ip=127.1.1.1")
                      .headers(headers).send().unwrap();

        // The hook should be queued
        assert_eq!(res.status, StatusCode::Ok);
        assert!(inst.processor_input().is_some());

        inst.stop();
        testing_env.cleanup();
    }
}
