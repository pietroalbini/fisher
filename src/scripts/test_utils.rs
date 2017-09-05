// Copyright (C) 2017 Pietro Albini
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
use std::fs;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::Arc;

use common::prelude::*;
use common::state::State;
use scripts::Script;
use utils::create_temp_dir;
use web::WebRequest;


pub struct TestEnv {
    state: Arc<State>,
    scripts_dir: PathBuf,
    to_cleanup: Vec<PathBuf>,
}

impl TestEnv {
    fn new() -> Result<Self> {
        let scripts_dir = create_temp_dir()?;

        Ok(TestEnv {
            state: Arc::new(State::new()),
            scripts_dir: scripts_dir.clone(),
            to_cleanup: vec![scripts_dir],
        })
    }

    pub fn state(&self) -> Arc<State> {
        self.state.clone()
    }

    pub fn tempdir(&mut self) -> Result<PathBuf> {
        let dir = create_temp_dir()?;
        self.to_cleanup.push(dir.clone());
        Ok(dir)
    }

    pub fn scripts_dir(&self) -> PathBuf {
        self.scripts_dir.clone()
    }

    pub fn create_script(&self, name: &str, content: &[&str]) -> Result<()> {
        self.create_script_into(&self.scripts_dir, name, content)
    }

    pub fn create_script_into(
        &self,
        path: &PathBuf,
        name: &str,
        content: &[&str],
    ) -> Result<()> {
        let path = path.join(name);

        let mut to_write = String::new();
        for line in content {
            to_write.push_str(line);
            to_write.push('\n');
        }

        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .mode(0o755)
            .open(&path)?
            .write(to_write.as_bytes())?;

        Ok(())
    }


    pub fn load_script(&self, name: &str) -> Result<Script> {
        let path = self.scripts_dir().join(name).to_str().unwrap().to_string();
        Ok(Script::load(name.into(), path, &self.state)?)
    }

    pub fn cleanup(&self) {
        for dir in &self.to_cleanup {
            let _ = fs::remove_dir_all(&dir);
        }
    }
}


pub fn dummy_web_request() -> WebRequest {
    WebRequest {
        headers: HashMap::new(),
        params: HashMap::new(),
        source: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        body: String::new(),
    }
}


pub fn test_wrapper<F: Fn(&mut TestEnv) -> Result<()>>(func: F) {
    let mut env = TestEnv::new().unwrap();

    let result = func(&mut env);
    env.cleanup();

    if let Err(error) = result {
        panic!("{}", error);
    }
}
