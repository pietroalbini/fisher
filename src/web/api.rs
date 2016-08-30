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

use std::collections::HashMap;
use std::net::SocketAddr;

use chan;
use nickel::{Nickel, HttpRouter, Options};
use nickel::status::StatusCode;
use hyper::method::Method;
use rustc_serialize::json::ToJson;

use hooks::Hook;
use processor::{RequestType, Job, ProcessorInput, SenderChan};
use web::responses::JsonResponse;
use web::utils::convert_request;


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

                if let Some(job_hook) = hook.validate(&request) {
                    // Do something different based on the request type
                    match job_hook.request_type(&request) {
                        // Don't do anything when it's only a ping
                        RequestType::Ping => {},

                        // Queue a job if the hook should be executed
                        RequestType::ExecuteHook => {
                            let job = Job::new(job_hook, request);
                            sender.send(ProcessorInput::Job(job));
                        },
                    }

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


#[cfg(test)]
mod tests {
    use std::io::Read;

    use hyper::status::StatusCode;
    use hyper::method::Method;
    use rustc_serialize::json::Json;

    use processor::{HealthDetails, ProcessorInput};
    use web::tests::TestInstance;


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
