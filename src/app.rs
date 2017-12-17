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

use std::net;
use std::sync::Arc;

use common::prelude::*;
use common::state::State;
use common::config::Config;

use scripts::{Blueprint, Repository, ScriptNamesIter, JobContext};
use processor::{Processor, ProcessorApi};
use web::WebApp;


#[derive(Debug)]
pub struct Fisher {
    config: Config,
    state: Arc<State>,
    scripts_repository: Repository,
    scripts_blueprint: Blueprint,
}

impl Fisher {
    pub fn new(config: Config) -> Result<Self> {
        let state = Arc::new(State::new());
        let mut scripts_blueprint = Blueprint::new(state.clone());
        let scripts_repository = scripts_blueprint.repository();

        // Collect scripts from the directory
        scripts_blueprint.collect_path(
            &config.scripts.path, config.scripts.subdirs,
        )?;

        Ok(Fisher {
            config,
            state: Arc::new(State::new()),
            scripts_blueprint,
            scripts_repository,
        })
    }

    pub fn script_names(&self) -> ScriptNamesIter {
        self.scripts_repository.names()
    }

    pub fn start(self) -> Result<RunningFisher> {
        // Finalize the hooks
        let repository = Arc::new(self.scripts_repository);

        let context = Arc::new(JobContext {
            environment: self.config.env,
            .. JobContext::default()
        });

        // Start the processor
        let processor = Processor::new(
            self.config.jobs.threads,
            repository.clone(),
            context,
            self.state.clone(),
        )?;
        let processor_api = processor.api();

        // Start the Web API
        let web_api = match WebApp::new(
            repository.clone(),
            self.config.http,
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

    pub fn reload_scripts(&mut self) -> Result<()> {
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
