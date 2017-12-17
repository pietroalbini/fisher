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
use std::collections::HashMap;

use common::prelude::*;
use common::state::State;
use common::config::{Config, HttpConfig};

use scripts::{Blueprint, Repository, JobContext};
use processor::{Processor, ProcessorApi};
use web::WebApp;


struct InnerApp {
    locked: bool,
    scripts_blueprint: Blueprint,
    processor: Processor<Repository>,
    http: Option<WebApp<ProcessorApi<Repository>>>,
}

impl InnerApp {
    fn new(config: &Config) -> Result<Self> {
        let state = Arc::new(State::new());
        let blueprint = Blueprint::new(state.clone());
        let repository = Arc::new(blueprint.repository());

        let processor = Processor::new(
            config.jobs.threads,
            repository.clone(),
            JobContext::default(),
            state.clone(),
        )?;

        Ok(InnerApp {
            locked: false,
            scripts_blueprint: blueprint,
            http: None,
            processor,
        })
    }

    fn restart_http_server(&mut self, config: &HttpConfig) -> Result<()> {
        // Stop the server if it's already running
        if let Some(http) = self.http.take() {
            http.stop();
        }

        let http = WebApp::new(
            Arc::new(self.scripts_blueprint.repository()),
            config,
            self.processor.api(),
        )?;

        // Lock the server if it was locked before
        if self.locked {
            http.lock();
        }

        self.http = Some(http);

        Ok(())
    }

    fn set_scripts_path<P: AsRef<Path>>(
        &mut self, path: P, subdirs: bool,
    ) -> Result<()> {
        self.scripts_blueprint.clear();
        self.scripts_blueprint.collect_path(path, subdirs)?;
        self.processor.api().cleanup()?;

        Ok(())
    }

    fn set_job_environment(&self, env: HashMap<String, String>) -> Result<()> {
        self.processor.api().update_context(JobContext {
            environment: env,
            .. JobContext::default()
        })?;
        Ok(())
    }

    fn http_addr(&self) -> Option<&SocketAddr> {
        if let Some(ref http) = self.http {
            Some(http.addr())
        } else {
            None
        }
    }

    fn lock(&mut self) -> Result<()> {
        if let Some(ref http) = self.http {
            http.lock();
        }
        self.processor.api().lock()?;

        self.locked = true;

        Ok(())
    }

    fn unlock(&mut self) -> Result<()> {
        self.processor.api().unlock()?;
        if let Some(ref http) = self.http {
            http.unlock();
        }

        self.locked = false;

        Ok(())
    }

    fn stop(mut self) -> Result<()> {
        if let Some(ref http) = self.http {
            http.lock();
        }

        self.processor.stop()?;

        if let Some(http) = self.http.take() {
            http.stop();
        }

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
        inner.set_job_environment(config.env.clone())?;
        inner.restart_http_server(&config.http)?;

        Ok(Fisher {
            config,
            inner,
        })
    }

    pub fn web_address(&self) -> Option<&SocketAddr> {
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
        // Restart the HTTP server if its configuration changed
        if self.config.http != new_config.http {
            self.inner.restart_http_server(&new_config.http)?;
        }

        // Update the job context if the environment is different
        if self.config.env != new_config.env {
            self.inner.set_job_environment(new_config.env.clone())?;
        }

        // Reload hooks, changing the script path
        self.inner.set_scripts_path(
            &new_config.scripts.path,
            new_config.scripts.subdirs,
        )?;

        self.config = new_config;

        Ok(())
    }

    pub fn stop(self) -> Result<()> {
        self.inner.stop()
    }
}
