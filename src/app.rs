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

use std::path::Path;
use std::net;
use std::sync::Arc;

use hooks::{self, HookNamesIter, Hooks, Hook};
use processor::ProcessorManager;
use web::WebApp;
use errors::FisherResult;


#[derive(Debug, Clone)]
pub struct FisherOptions {
    pub bind: String,
    pub hooks_dir: String,
    pub max_threads: u16,
    pub enable_health: bool,
    pub behind_proxies: Option<u8>,
}

impl FisherOptions {

    #[cfg(test)]
    pub fn defaults() -> Self {
        FisherOptions {
            bind: "127.0.0.1:8000".to_string(),
            hooks_dir: "hooks/".to_string(),
            max_threads: 1,
            enable_health: true,
            behind_proxies: None,
        }
    }
}


pub struct Fisher<'a> {
    options: &'a FisherOptions,
    hooks: Hooks,
}

impl<'a> Fisher<'a> {

    pub fn new(options: &'a FisherOptions) -> Self {
        Fisher {
            options: options,
            hooks: Hooks::new(),
        }
    }

    pub fn add_hook(&mut self, name: String, hook: Hook) {
        self.hooks.insert(name, hook);
    }

    pub fn collect_hooks<P: AsRef<Path>>(&mut self, path: P) -> FisherResult<()> {
        let mut hooks = hooks::collect(path)?;
        for (name, hook) in hooks.drain() {
            self.add_hook(name, hook);
        }

        Ok(())
    }

    pub fn hook_names<'b>(&'b self) -> HookNamesIter<'b> {
        self.hooks.names()
    }

    pub fn start(self) -> FisherResult<RunningFisher> {
        // Finalize the hooks
        let hooks = Arc::new(self.hooks);

        // Initialize the state
        let mut processor = ProcessorManager::new();
        let mut web_api = WebApp::new();

        // Start the processor
        processor.start(self.options.max_threads, hooks.clone());
        let processor_input = processor.input().unwrap();

        // Start the Web API
        let listening;
        match web_api.listen(
            hooks.clone(), self.options, processor_input
        ) {
            Ok(socket) => {
                listening = socket;
            },
            Err(error) => {
                // Be sure to stop the processor
                processor.stop();

                return Err(error);
            },
        }

        Ok(RunningFisher::new(
            processor,
            web_api,
            listening,
        ))
    }
}


pub struct RunningFisher {
    processor: ProcessorManager,
    web_api: WebApp,
    web_address: net::SocketAddr,
}

impl RunningFisher {

    fn new(processor: ProcessorManager, web_api: WebApp,
           web_address: net::SocketAddr) -> Self {
        RunningFisher {
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
