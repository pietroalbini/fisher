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

use std::sync::{Arc, Mutex, mpsc};

use requests::{Request, RequestType};
use hooks::Hooks;
use jobs::Job;
use processor::ProcessorInput;
use web::responses::Response;
use logger::{Logger, LogEvent};


#[derive(Clone)]
pub struct WebApi {
    processor_input: Arc<Mutex<mpsc::Sender<ProcessorInput>>>,
    hooks: Arc<Hooks>,
    logger: Arc<Mutex<Logger>>,

    health_enabled: bool,
}

impl WebApi {

    pub fn new(processor_input: mpsc::Sender<ProcessorInput>,
               hooks: Arc<Hooks>, health_enabled: bool, logger: Logger)
               -> Self {
        WebApi {
            processor_input: Arc::new(Mutex::new(processor_input)),
            hooks: hooks,
            health_enabled: health_enabled,
            logger: Arc::new(Mutex::new(logger)),
        }
    }

    pub fn process_hook(&self, req: &Request, args: Vec<String>) -> Response {
        let hook_name = &args[0];

        // Check if the hook exists
        let hook;
        if let Some(found) = self.hooks.get(hook_name) {
            hook = found;
        } else {
            return Response::NotFound;
        }

        // Validate the hook
        let (request_type, provider) = hook.validate(req);

        // Change behavior based on the request type
        match request_type {

            RequestType::Ping => {
                self.logger.lock().unwrap()
                           .log(LogEvent::PingReceived(hook.name().into()));

                Response::Ok
            },

            // Queue a job if the hook should be executed
            RequestType::ExecuteHook => {
                let job = Job::new(hook.clone(), provider, req.clone());
                self.processor_input.lock().unwrap()
                    .send(ProcessorInput::Job(job)).unwrap();

                Response::Ok
            },

            RequestType::Invalid => Response::Forbidden,
        }
    }

    pub fn get_health(&self, _req: &Request, _args: Vec<String>) -> Response {
        if self.health_enabled {
            // Create a channel to communicate with the processor
            let (details_send, details_recv) = mpsc::channel();

            // Get the details from the processor
            self.processor_input.lock().unwrap().send(
                ProcessorInput::HealthStatus(details_send)
            ).unwrap();
            let details = details_recv.recv().unwrap();

            Response::HealthStatus(details)
        } else {
            Response::Forbidden
        }
    }
}
