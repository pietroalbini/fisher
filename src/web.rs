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

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;

use chan;
use nickel;
use nickel::{Nickel, HttpRouter, Options};
use nickel::status::StatusCode;
use hyper::method::Method;
use hyper::uri::RequestUri;
use url::form_urlencoded;
use rustc_serialize::json::{Json, ToJson};

use hooks::Hook;
use processor::{HealthDetails, Request, Job, ProcessorInput, SenderChan};


enum JsonResponse {
    NotFound,
    Forbidden,
    Ok,
    HealthStatus(HealthDetails),
}

impl ToJson for JsonResponse {

    fn to_json(&self) -> Json {
        let mut map = BTreeMap::new();

        map.insert("status".to_string(), match *self {
            JsonResponse::NotFound => "not_found",
            JsonResponse::Forbidden => "forbidden",
            JsonResponse::Ok => "ok",
            JsonResponse::HealthStatus(..) => "ok"
        }.to_string().to_json());

        if let JsonResponse::HealthStatus(ref details) = *self {
            map.insert("result".to_string(), details.to_json());
        }

        Json::Object(map)
    }
}


pub struct WebAPI {
    stop_chan: Option<chan::Sender<()>>,
    sender_chan: Option<SenderChan>,
    stop: bool,
    hooks: HashMap<String, Hook>,
}

impl WebAPI {

    pub fn new(hooks: HashMap<String, Hook>) -> WebAPI {
        WebAPI {
            stop_chan: None,
            sender_chan: None,
            stop: false,
            hooks: hooks,
        }
    }

    pub fn listen(&mut self, bind: &str, enable_health: bool,
                  sender: SenderChan) -> Result<SocketAddr, String> {
        // Store the sender channel
        self.sender_chan = Some(sender);

        // This is to fix lifetime issues with the thread below
        let bind = bind.to_string();

        let app = self.configure_nickel(enable_health);

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

    fn configure_nickel(&self, enable_health: bool) -> Nickel {
        let mut app = Nickel::new();

        // Disable the default message nickel prints on stdout
        app.options = Options::default().output_on_listen(false);

        for method in &[Method::Get, Method::Post] {
            // Make the used things owned
            let method = method.clone();
            let sender = self.sender_chan.clone().unwrap();
            let hooks = self.hooks.clone();

            // This middleware processes incoming hooks
            app.add_route(method, "/hook/:hook", middleware! { |req, mut res|
                let hook_name = req.param("hook").unwrap().to_string();

                // Ignore requests without a valid hook
                if hook_name == "".to_string() {
                    return res.next_middleware();
                }

                // Ignore requests with non-existent hooks
                let hook: Hook;
                if let Some(found) = hooks.get(&hook_name) {
                    hook = found.clone();
                } else {
                    return res.next_middleware();
                }

                let request = convert_request(&req);

                if let Some(job_hook) = hook.validate(&request.clone()) {
                    // If the hook is valid, create a new job and queue it
                    let job = Job::new(job_hook, request);
                    sender.send(ProcessorInput::Job(job));

                    JsonResponse::Ok.to_json()
                } else {
                    // Else send a great 403 Forbidden
                    res.set(StatusCode::Forbidden);
                    JsonResponse::Forbidden.to_json()
                }
            });
        }


        // Health reporting can be disabled by the user
        if enable_health {
            let sender = self.sender_chan.clone().unwrap();

            app.get("/health", middleware! {
                let (details_send, details_recv) = chan::async();

                // Get the details from the processor
                sender.send(ProcessorInput::HealthStatus(details_send));
                let details = details_recv.recv().unwrap();

                JsonResponse::HealthStatus(details).to_json()
            });
        } else {
            app.get("/health", middleware! { |_req, mut res|
                res.set(StatusCode::Forbidden);

                JsonResponse::Forbidden.to_json()
            });
        }


        // This middleware provides a basic Not found page
        app.utilize(middleware! { |_req, mut res|
            res.set(StatusCode::NotFound);

            JsonResponse::NotFound.to_json()
        });

        app
    }

}


fn convert_request(req: &nickel::Request) -> Request {
    let source = req.origin.remote_addr.clone();

    // Convert headers from the hyper representation to strings
    let mut headers = HashMap::new();
    for header in req.origin.headers.iter() {
        headers.insert(header.name().to_string(), header.value_string());
    }

    let params = params_from_request(req);

    Request {
        source: source,
        headers: headers,
        params: params,
    }
}


fn params_from_request(req: &nickel::Request) -> HashMap<String, String> {
    let ref uri = req.origin.uri;

    let query_string = match *uri {
        RequestUri::AbsoluteUri(ref url) => Some(url.query()),
        RequestUri::AbsolutePath(ref s) => Some(s.splitn(2, '?').nth(1)),
        _ => None,
    };

    match query_string {
        Some(path) => {
            // Don't do anything if there is no query string
            if path.is_none() {
                return HashMap::new();
            }
            let path = path.unwrap();

            let mut hashmap = HashMap::new();
            for (a, b) in form_urlencoded::parse(path.as_bytes()).into_owned() {
                hashmap.insert(a, b);
            }
            hashmap
        },
        None => HashMap::new(),
    }
}


#[cfg(test)]
pub mod tests {
    use std::path::PathBuf;
    use std::fs;

    use chan;
    use hyper::client as hyper;
    use hyper::method::Method;

    use super::WebAPI;
    use hooks;
    use hooks::tests::create_sample_hooks;
    use processor::{HealthDetails, ProcessorInput};


    pub struct TestInstance {
        inst: WebAPI,
        url: String,
        client: hyper::Client,

        tempdir: PathBuf,
        input_recv: chan::Receiver<ProcessorInput>,
    }

    impl TestInstance {

        pub fn new(health: bool) -> Self {
            // Create a new instance of WebAPI
            let tempdir = create_sample_hooks();
            let mut inst = WebAPI::new(hooks::collect(
                &tempdir.to_str().unwrap().to_string()
            ).unwrap());

            // Create the input channel
            let (input_send, input_recv) = chan::async();

            // Start the web server
            let addr = inst.listen("127.0.0.1:0", health, input_send).unwrap();

            // Create the HTTP client
            let url = format!("http://{}", addr);
            let client = hyper::Client::new();

            TestInstance {
                inst: inst,
                url: url,
                client: client,

                tempdir: tempdir,
                input_recv: input_recv,
            }
        }

        pub fn close(&mut self) {
            // Close the instance
            self.inst.stop();

            // Remove the directory
            fs::remove_dir_all(&self.tempdir).unwrap();
        }

        pub fn request(&mut self, method: Method, url: &str)
                       -> hyper::RequestBuilder {
            // Create the HTTP request
            self.client.request(method, &format!("{}{}", self.url, url))
        }

        pub fn processor_input(&self) -> Option<ProcessorInput> {
            let input_recv = &self.input_recv;

            // This returns Some only if there is something right now
            chan_select! {
                default => {
                    return None;
                },
                input_recv.recv() -> input => {
                    return Some(input.unwrap());
                },
            };
        }

        pub fn next_health(&self, details: HealthDetails) -> NextHealthCheck {
            let input_chan = self.input_recv.clone();
            let (result_send, result_recv) = chan::async();

            ::std::thread::spawn(move || {
                let input = input_chan.recv().unwrap();

                if let ProcessorInput::HealthStatus(ref sender) = input {
                    // Send the HealthDetails we want
                    sender.send(details);

                    // Everything was OK
                    result_send.send(None);
                } else {
                    result_send.send(Some(
                        "Wrong kind of ProcessorInput received!".to_string()
                    ));
                }
            });

            NextHealthCheck::new(result_recv)
        }
    }


    pub struct NextHealthCheck {
        result_recv: chan::Receiver<Option<String>>,
    }

    impl NextHealthCheck {

        fn new(result_recv: chan::Receiver<Option<String>>) -> Self {
            NextHealthCheck {
                result_recv: result_recv,
            }
        }

        fn check(&self) {
            let result_recv = &self.result_recv;

            chan_select! {
                default => {
                    panic!("No ProcessorInput received!");
                },
                result_recv.recv() -> result => {
                    // Forward panics
                    if let Some(message) = result.unwrap() {
                        panic!(message);
                    }
                },
            };
        }
    }


    mod json_responses {
        use rustc_serialize::json::ToJson;

        use processor::HealthDetails;
        use super::super::JsonResponse;


        #[test]
        fn test_not_found() {
            let response = JsonResponse::NotFound.to_json();

            // The result must be an object
            let obj = response.as_object().unwrap();

            // The status must be "not_found"
            assert_eq!(
                obj.get("status").unwrap().as_string().unwrap(),
                "not_found".to_string()
            );
        }


        #[test]
        fn test_forbidden() {
            let response = JsonResponse::Forbidden.to_json();

            // The result must be an object
            let obj = response.as_object().unwrap();

            // The status must be "forbidden"
            assert_eq!(
                obj.get("status").unwrap().as_string().unwrap(),
                "forbidden".to_string()
            );
        }


        #[test]
        fn test_ok() {
            let response = JsonResponse::Ok.to_json();

            // The result must be an object
            let obj = response.as_object().unwrap();

            // The status must be "ok"
            assert_eq!(
                obj.get("status").unwrap().as_string().unwrap(),
                "ok".to_string()
            );
        }


        #[test]
        fn test_health_status() {
            let response = JsonResponse::HealthStatus(HealthDetails {
                active_jobs: 1,
                queue_size: 2,
            }).to_json();

            // The result must be an object
            let obj = response.as_object().unwrap();

            // The status must be "ok"
            assert_eq!(
                obj.get("status").unwrap().as_string().unwrap(),
                "ok".to_string()
            );

            // It must have an object called "result"
            let result = obj.get("result").unwrap().as_object().unwrap();

            // The result must contain "active_jobs" and "queue_size"
            assert_eq!(
                result.get("active_jobs").unwrap().as_u64().unwrap(),
                1 as u64
            );
            assert_eq!(
                result.get("queue_size").unwrap().as_u64().unwrap(),
                2 as u64
            );
        }

    }


    mod web_api {
        use std::io::Read;

        use hyper::status::StatusCode;
        use hyper::method::Method;
        use rustc_serialize::json::Json;

        use processor::{HealthDetails, ProcessorInput};
        use super::TestInstance;


        #[test]
        fn test_startup() {
            let mut inst = TestInstance::new(true);

            // Test if the Web API is working fine
            let res = inst.request(Method::Get, "/").send().unwrap();
            assert_eq!(res.status, StatusCode::NotFound);

            inst.close();
        }

        #[test]
        fn test_hook_call() {
            let mut inst = TestInstance::new(true);

            // It shouldn't be possible to call a non-existing hook
            let res = inst.request(Method::Get, "/hook/invalid")
                          .send().unwrap();
            assert_eq!(res.status, StatusCode::NotFound);
            assert!(inst.processor_input().is_none());

            // Call the example hook without authorization
            let res = inst.request(Method::Get, "/hook/example.sh")
                          .send().unwrap();
            assert_eq!(res.status, StatusCode::Forbidden);
            assert!(inst.processor_input().is_none());

            // Call the example hook with authorization
            let res = inst.request(Method::Get, "/hook/example?secret=12345")
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

            inst.close();
        }

        #[test]
        fn test_health_disabled() {
            // Create the instance with disabled health status
            let mut inst = TestInstance::new(false);

            // It shouldn't be possible to get the health status
            let res = inst.request(Method::Get, "/health").send().unwrap();
            assert_eq!(res.status, StatusCode::Forbidden);

            inst.close();
        }

        #[test]
        fn test_health_enabled() {
            // Create the instance with enabled health status
            let mut inst = TestInstance::new(true);

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

            inst.close();
        }
    }
}
