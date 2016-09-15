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

use std::net;

use hooks::{Hooks, Hook};
use processor::ProcessorManager;
use web::WebApi;
use errors::{ErrorKind, FisherResult};


#[derive(Debug, Clone)]
pub struct FisherOptions {
    pub bind: String,
    pub hooks_dir: String,
    pub max_threads: u16,
    pub enable_health: bool,
}

impl FisherOptions {

    #[cfg(test)]
    pub fn defaults() -> Self {
        FisherOptions {
            bind: "127.0.0.1:8000".to_string(),
            hooks_dir: "hooks/".to_string(),
            max_threads: 1,
            enable_health: true,
        }
    }
}


pub struct AppFactory<'a> {
    options: &'a FisherOptions,
    hooks: Hooks,
}

impl<'a> AppFactory<'a> {

    pub fn new(options: &'a FisherOptions) -> Self {
        AppFactory {
            options: options,
            hooks: Hooks::new(),
        }
    }

    pub fn add_hook(&mut self, name: &str, hook: Hook) {
        self.hooks.insert(name.to_string(), hook);
    }

    pub fn start(&'a mut self) -> FisherResult<RunningApp> {
        // Initialize the state
        let mut processor = ProcessorManager::new();
        let mut web_api = WebApi::new(&self.hooks);

        // Start the processor
        processor.start(self.options.max_threads);
        let processor_input = processor.sender().unwrap();

        // Start the Web API
        let listening;
        match web_api.listen(self.options, processor_input) {
            Ok(socket) => {
                listening = socket;
            },
            Err(error) => {
                // Be sure to stop the processor
                processor.stop();

                return Err(
                    ErrorKind::WebApiStartFailed(error.clone()).into()
                );
            },
        }

        Ok(RunningApp::new(
            processor,
            web_api,
            listening,
        ))
    }
}


pub struct RunningApp<'a> {
    processor: ProcessorManager,
    web_api: WebApi<'a>,
    web_address: net::SocketAddr,
}

impl<'a> RunningApp<'a> {

    fn new(processor: ProcessorManager, web_api: WebApi<'a>,
           web_address: net::SocketAddr) -> Self {
        RunningApp {
            processor: processor,
            web_api: web_api,
            web_address: web_address,
        }
    }

    pub fn web_address(&self) -> &net::SocketAddr {
        &self.web_address
    }

    pub fn stop(&mut self) {
        self.web_api.stop();
        self.processor.stop();
    }
}
