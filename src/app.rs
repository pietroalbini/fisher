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

use std::collections::HashMap;
use std::path::Path;
use std::net;
use std::sync::Arc;

use fisher_common::prelude::*;
use fisher_common::state::State;

use hooks::{HookNamesIter, Hooks, HooksBlueprint, Hook};
use processor::Processor;
use web::WebApp;
use utils;


pub trait IntoHook {
    fn into_hook(self) -> Arc<Hook>;
}

impl IntoHook for Hook {
    fn into_hook(self) -> Arc<Hook> {
        Arc::new(self)
    }
}

impl IntoHook for Arc<Hook> {
    fn into_hook(self) -> Arc<Hook> {
        self
    }
}


#[derive(Debug)]
pub struct Fisher<'a> {
    pub max_threads: u16,
    pub behind_proxies: u8,
    pub bind: &'a str,
    pub enable_health: bool,

    state: Arc<State>,
    hooks: Hooks,
    hooks_blueprint: HooksBlueprint,
    environment: HashMap<String, String>,
}

impl<'a> Fisher<'a> {

    pub fn new() -> Self {
        let state = Arc::new(State::new());
        let hooks_blueprint = HooksBlueprint::new(state.clone());
        let hooks = hooks_blueprint.hooks();

        Fisher {
            max_threads: 1,
            behind_proxies: 0,
            bind: "127.0.0.1:8000",
            enable_health: true,

            state: Arc::new(State::new()),
            hooks: hooks,
            hooks_blueprint: hooks_blueprint,
            environment: HashMap::new(),
        }
    }

    pub fn env(&mut self, key: String, value: String) {
        let _ = self.environment.insert(key, value);
    }

    pub fn raw_env(&mut self, env: &str) -> Result<()> {
        let (key, value) = utils::parse_env(env)?;
        self.env(key.into(), value.into());
        Ok(())
    }

    pub fn add_hook<H: IntoHook>(&mut self, hook: H) -> Result<()> {
        self.hooks_blueprint.insert(hook.into_hook())?;
        Ok(())
    }

    pub fn collect_hooks<P: AsRef<Path>>(&mut self, path: P, recursive: bool)
                                         -> Result<()> {
        self.hooks_blueprint.collect_path(path, recursive)?;
        Ok(())
    }

    pub fn hook_names<'b>(&'b self) -> HookNamesIter<'b> {
        self.hooks.names()
    }

    pub fn start(self) -> Result<RunningFisher> {
        // Finalize the hooks
        let hooks = Arc::new(self.hooks);

        // Start the processor
        let processor = Processor::new(
            self.max_threads, hooks.clone(), self.environment,
            self.state.clone(),
        )?;
        let processor_api = processor.api();

        // Start the Web API
        let web_api = match WebApp::new(
            hooks.clone(), self.enable_health, self.behind_proxies, self.bind,
            processor_api,
        ) {
            Ok(socket) => socket,
            Err(error) => {
                // Be sure to stop the processor
                processor.stop()?;

                return Err(error);
            },
        };

        Ok(RunningFisher::new(
            processor,
            web_api,
            self.hooks_blueprint,
        ))
    }
}


pub struct RunningFisher {
    processor: Processor,
    web_api: WebApp,
    hooks_blueprint: HooksBlueprint,
}

impl RunningFisher {

    fn new(processor: Processor, web_api: WebApp,
           hooks_blueprint: HooksBlueprint) -> Self {
        RunningFisher {
            processor: processor,
            web_api: web_api,
            hooks_blueprint: hooks_blueprint,
        }
    }

    pub fn web_address(&self) -> &net::SocketAddr {
        self.web_api.addr()
    }

    pub fn reload(&mut self) -> Result<()> {
        let processor = self.processor.api();

        self.web_api.lock();
        processor.lock()?;

        let result = self.hooks_blueprint.reload();
        if result.is_ok() {
            processor.cleanup()?;
        }

        processor.unlock()?;
        self.web_api.unlock();

        result
    }

    pub fn stop(self) -> Result<()> {
        self.web_api.lock();
        self.processor.stop()?;
        self.web_api.stop();

        Ok(())
    }
}
