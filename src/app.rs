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

use common::prelude::*;
use common::state::State;

use scripts::{Blueprint, Repository, Script, ScriptNamesIter};
use jobs::Context;
use processor::{Processor, ProcessorApi};
use utils;
use web::WebApp;


pub trait IntoScript {
    fn into_script(self) -> Arc<Script>;
}

impl IntoScript for Script {
    fn into_script(self) -> Arc<Script> {
        Arc::new(self)
    }
}

impl IntoScript for Arc<Script> {
    fn into_script(self) -> Arc<Script> {
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
    scripts_repository: Repository,
    scripts_blueprint: Blueprint,
    environment: HashMap<String, String>,
}

impl<'a> Fisher<'a> {
    pub fn new() -> Self {
        let state = Arc::new(State::new());
        let scripts_blueprint = Blueprint::new(state.clone());
        let scripts_repository = scripts_blueprint.repository();

        Fisher {
            max_threads: 1,
            behind_proxies: 0,
            bind: "127.0.0.1:8000",
            enable_health: true,

            state: Arc::new(State::new()),
            scripts_blueprint,
            scripts_repository,
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

    pub fn add_script<S: IntoScript>(&mut self, script: S) -> Result<()> {
        self.scripts_blueprint.insert(script.into_script())?;
        Ok(())
    }

    pub fn collect_scripts<P: AsRef<Path>>(
        &mut self,
        path: P,
        recursive: bool,
    ) -> Result<()> {
        self.scripts_blueprint.collect_path(path, recursive)?;
        Ok(())
    }

    pub fn script_names(&self) -> ScriptNamesIter {
        self.scripts_repository.names()
    }

    pub fn start(self) -> Result<RunningFisher> {
        // Finalize the hooks
        let repository = Arc::new(self.scripts_repository);

        let context = Arc::new(Context {
            environment: self.environment,
        });

        // Start the processor
        let processor = Processor::new(
            self.max_threads,
            repository.clone(),
            context,
            self.state.clone(),
        )?;
        let processor_api = processor.api();

        // Start the Web API
        let web_api = match WebApp::new(
            repository.clone(),
            self.enable_health,
            self.behind_proxies,
            self.bind,
            processor_api,
        ) {
            Ok(socket) => socket,
            Err(error) => {
                // Be sure to stop the processor
                processor.stop()?;

                return Err(error);
            }
        };

        Ok(RunningFisher::new(
            processor,
            web_api,
            self.scripts_blueprint,
        ))
    }
}


pub struct RunningFisher {
    processor: Processor<Repository>,
    web_api: WebApp<ProcessorApi<Repository>>,
    scripts_blueprint: Blueprint,
}

impl RunningFisher {
    fn new(
        processor: Processor<Repository>,
        web_api: WebApp<ProcessorApi<Repository>>,
        scripts_blueprint: Blueprint,
    ) -> Self {
        RunningFisher {
            processor,
            web_api,
            scripts_blueprint,
        }
    }

    pub fn web_address(&self) -> &net::SocketAddr {
        self.web_api.addr()
    }

    pub fn reload(&mut self) -> Result<()> {
        let processor = self.processor.api();

        self.web_api.lock();
        processor.lock()?;

        let result = self.scripts_blueprint.reload();
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
