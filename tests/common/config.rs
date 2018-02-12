// Copyright (C) 2018 Pietro Albini
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
use std::fs::File;
use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;

use common::env::TestingEnv;
use common::prelude::*;


#[derive(Serialize)]
pub struct Config<'a> {
    pub http: HttpConfig,
    pub scripts: ScriptsConfig,
    pub jobs: JobsConfig,
    pub env: HashMap<String, String>,

    #[serde(skip)]
    testing_env: &'a TestingEnv,
}

impl<'a> Config<'a> {
    pub fn new(testing_env: &'a TestingEnv) -> Config {
        Config {
            http: HttpConfig {
                behind_proxies: 0,
                bind: "127.0.0.1:0".parse().unwrap(),
                rate_limit: "100000000/1s".into(),
                health_endpoint: true,
            },
            scripts: ScriptsConfig {
                path: testing_env.scripts_path().to_str().unwrap().to_string(),
                recursive: false,
            },
            jobs: JobsConfig {
                threads: 1,
            },
            env: HashMap::new(),

            testing_env,
        }
    }

    pub fn save(&self) -> Result<PathBuf> {
        let path = self.testing_env.tempdir()?.join("config.toml");
        write!(File::create(&path)?, "{}", ::toml::to_string(self)?)?;
        Ok(path)
    }
}


#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct HttpConfig {
    pub behind_proxies: u8,
    pub bind: SocketAddr,
    pub rate_limit: String,
    pub health_endpoint: bool,
}


#[derive(Serialize)]
pub struct JobsConfig {
    pub threads: u16,
}


#[derive(Serialize)]
pub struct ScriptsConfig {
    pub path: String,
    pub recursive: bool,
}
