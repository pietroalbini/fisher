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
use std::sync::Arc;

use hooks::{Hooks, Hook};
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


enum HooksContainer {
    Mutable(Hooks),
    Immutable(Arc<Hooks>),
}

impl HooksContainer {

    fn new() -> HooksContainer {
        HooksContainer::Mutable(Hooks::new())
    }

    fn insert(&mut self, name: String, value: Hook) {
        if let HooksContainer::Mutable(ref mut hooks) = *self {
            hooks.insert(name, value);
        } else {
            panic!("You can't add hooks at runtime");
        }
    }

    fn finalize(&self) -> HooksContainer {
        if let &HooksContainer::Mutable(ref hooks) = self {
            HooksContainer::Immutable(Arc::new(hooks.clone()))
        } else {
            panic!("Already finalized!")
        }
    }

    fn reference(&self) -> Arc<Hooks> {
        if let HooksContainer::Immutable(ref hooks) = *self {
            hooks.clone()
        } else {
            panic!("You can't get a reference before finalizing");
        }
    }
}


pub struct AppFactory<'a> {
    options: &'a FisherOptions,
    hooks: HooksContainer,
}

impl<'a> AppFactory<'a> {

    pub fn new(options: &'a FisherOptions) -> Self {
        AppFactory {
            options: options,
            hooks: HooksContainer::new(),
        }
    }

    pub fn add_hook(&mut self, name: String, hook: Hook) {
        self.hooks.insert(name, hook);
    }

    pub fn start(&'a mut self) -> FisherResult<RunningApp> {
        // Finalize the hooks
        self.hooks = self.hooks.finalize();

        // Initialize the state
        let mut processor = ProcessorManager::new();
        let mut web_api = WebApp::new();

        // Start the processor
        processor.start(self.options.max_threads, self.hooks.reference());
        let processor_input = processor.sender().unwrap();

        // Start the Web API
        let listening;
        match web_api.listen(
            self.hooks.reference(), self.options, processor_input
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

        Ok(RunningApp::new(
            processor,
            web_api,
            listening,
        ))
    }
}


pub struct RunningApp {
    processor: ProcessorManager,
    web_api: WebApp,
    web_address: net::SocketAddr,
}

impl RunningApp {

    fn new(processor: ProcessorManager, web_api: WebApp,
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
