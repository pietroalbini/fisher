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

use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use common::prelude::*;
use common::config::RateLimitConfig;

use requests::{Request, RequestType};
use scripts::{Repository, Job};
use web::rate_limits::RateLimiter;
use web::responses::Response;


#[derive(Clone)]
pub struct WebApi<A: ProcessorApiTrait<Repository>> {
    processor: Arc<Mutex<A>>,
    hooks: Arc<Repository>,
    locked: Arc<AtomicBool>,
    limiter: Arc<Mutex<RateLimiter<IpAddr>>>,

    health_enabled: bool,
}

impl<A: ProcessorApiTrait<Repository>> WebApi<A> {
    pub fn new(
        processor: A,
        hooks: Arc<Repository>,
        locked: Arc<AtomicBool>,
        rate_limit_config: RateLimitConfig,
        health_enabled: bool,
    ) -> Self {
        let limiter = Arc::new(Mutex::new(RateLimiter::new(
            rate_limit_config.allowed,
            rate_limit_config.interval.as_u64(),
        )));

        WebApi {
            processor: Arc::new(Mutex::new(processor)),
            hooks, locked, limiter, health_enabled,
        }
    }

    pub fn process_hook(&self, req: &Request, args: Vec<String>) -> Response {
        let hook_name = &args[0];

        // Don't process hooks if the web api is locked
        if self.locked.load(Ordering::Relaxed) {
            return Response::Unavailable;
        }

        // Check if the user is not rate limited
        if let Ok(r) = req.web() {
            let limited = self.limiter.lock().unwrap().is_limited(&r.source);
            if let Some(until) = limited {
                return Response::TooManyRequests(until);
            }
        }

        // Check if the hook exists
        let hook;
        if let Some(found) = self.hooks.get_by_name(hook_name) {
            hook = found;
        } else {
            return Response::NotFound;
        }

        // Validate the hook
        let (request_type, provider) = hook.validate(req);

        // Change behavior based on the request type
        match request_type {
            // Don't do anything if it's only a ping
            RequestType::Ping => Response::Ok,

            // Queue a job if the hook should be executed
            RequestType::ExecuteHook => {
                let job = Job::new(hook.clone(), provider, req.clone());
                self.processor
                    .lock()
                    .unwrap()
                    .queue(job, hook.priority())
                    .unwrap();

                Response::Ok
            },

            RequestType::Invalid => {
                // Increment the limits for the user
                if let Ok(r) = req.web() {
                    self.limiter.lock().unwrap().increment(r.source);
                }

                Response::Forbidden
            },
        }
    }

    pub fn get_health(&self, _req: &Request, _args: Vec<String>) -> Response {
        if self.health_enabled {
            Response::HealthStatus(
                self.processor.lock().unwrap().health_details().unwrap(),
            )
        } else {
            Response::Forbidden
        }
    }
}
