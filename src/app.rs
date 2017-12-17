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

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use common::prelude::*;
use common::state::State;
use common::config::Config;

use scripts::{Blueprint, Repository, JobContext};
use processor::{Processor, ProcessorApi};
use web::WebApp;


struct InnerApp {
    scripts_blueprint: Blueprint,
    processor: Processor<Repository>,
    http: WebApp<ProcessorApi<Repository>>,
}

impl InnerApp {
    fn new(config: &Config) -> Result<Self> {
        let state = Arc::new(State::new());
        let blueprint = Blueprint::new(state.clone());
        let repository = Arc::new(blueprint.repository());

        let processor = Processor::new(
            config.jobs.threads,
            repository.clone(),
            Arc::new(JobContext {
                environment: config.env.clone(),
                .. JobContext::default()
            }),
            state.clone(),
        )?;

        Ok(InnerApp {
            scripts_blueprint: blueprint,
            http: WebApp::new(
                repository.clone(),
                &config.http,
                processor.api(),
            )?,
            processor,
        })
    }

    fn set_scripts_path<P: AsRef<Path>>(
        &mut self, path: P, subdirs: bool,
    ) -> Result<()> {
        self.scripts_blueprint.clear();
        self.scripts_blueprint.collect_path(path, subdirs)?;
        self.processor.api().cleanup()?;

        Ok(())
    }

    fn http_addr(&self) -> &SocketAddr {
        self.http.addr()
    }

    fn lock(&self) -> Result<()> {
        self.http.lock();
        self.processor.api().lock()?;

        Ok(())
    }

    fn unlock(&self) -> Result<()> {
        self.processor.api().unlock()?;
        self.http.unlock();

        Ok(())
    }

    fn stop(self) -> Result<()> {
        self.http.lock();
        self.processor.stop()?;
        self.http.stop();

        Ok(())
    }
}


pub struct Fisher {
    config: Config,
    inner: InnerApp,
}

impl Fisher {
    pub fn new(config: Config) -> Result<Self> {
        let mut inner = InnerApp::new(&config)?;
        inner.set_scripts_path(&config.scripts.path, config.scripts.subdirs)?;

        Ok(Fisher {
            config,
            inner,
        })
    }

    pub fn web_address(&self) -> &SocketAddr {
        self.inner.http_addr()
    }

    pub fn reload(&mut self, new_config: Config) -> Result<()> {
        // Ensure Fisher is unlocked even if the reload fails
        self.inner.lock()?;
        let result = self.reload_inner(new_config);
        self.inner.unlock()?;

        result
    }

    fn reload_inner(&mut self, new_config: Config) -> Result<()> {
        // Reload hooks, changing the script path
        self.inner.set_scripts_path(
            &new_config.scripts.path,
            new_config.scripts.subdirs,
        )?;

        Ok(())
    }

    pub fn stop(self) -> Result<()> {
        self.inner.stop()
    }
}
