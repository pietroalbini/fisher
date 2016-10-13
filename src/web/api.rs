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

use std::sync::Arc;

use chan;

use requests::{Request, RequestType};
use hooks::Hooks;
use jobs::Job;
use processor::ProcessorInput;
use web::responses::Response;


#[derive(Clone)]
pub struct WebApi {
    processor_input: chan::Sender<ProcessorInput>,
    hooks: Arc<Hooks>,

    health_enabled: bool,
}

impl WebApi {

    pub fn new(processor_input: chan::Sender<ProcessorInput>,
               hooks: Arc<Hooks>, health_enabled: bool) -> Self {
        WebApi {
            processor_input: processor_input,
            hooks: hooks,
            health_enabled: health_enabled,
        }
    }

    pub fn process_hook(&self, req: Request, hook_name: String) -> Response {
        // Check if the hook exists
        let hook;
        if let Some(found) = self.hooks.get(&hook_name) {
            hook = found;
        } else {
            return Response::NotFound;
        }

        // Validate the hook
        let (validated, provider) = hook.validate(&req);
        if validated {
            // Get the request type
            let request_type = if let Some(ref real_provider) = provider {
                real_provider.request_type(&req)
            } else {
                RequestType::ExecuteHook
            };

            // Change behavior based on the request type
            match request_type {
                // Don't do anything if it's only a ping
                RequestType::Ping => Response::Ok,

                // Queue a job if the hook should be executed
                RequestType::ExecuteHook => {
                    let job = Job::new(hook.clone(), provider, req);
                    self.processor_input.send(ProcessorInput::Job(job));

                    Response::Ok
                },

                // Return a "forbidden" if the request is meant to be internal
                RequestType::Internal => Response::Forbidden,
            }
        } else {
            Response::Forbidden
        }
    }

    pub fn get_health(&self, _req: Request) -> Response {
        if self.health_enabled {
            // Create a channel to communicate with the processor
            let (details_send, details_recv) = chan::async();

            // Get the details from the processor
            self.processor_input.send(
                ProcessorInput::HealthStatus(details_send)
            );
            let details = details_recv.recv().unwrap();

            Response::HealthStatus(details)
        } else {
            Response::Forbidden
        }
    }

    pub fn not_found(&self, _req: Request) -> Response {
        Response::NotFound
    }
}
