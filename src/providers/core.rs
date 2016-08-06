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

use std::collections::HashMap;

use rustc_serialize::json;


pub type ValidatorFunc = fn(json::Json) -> bool;
pub type EnvFunc = fn(json::Json) -> HashMap<String, String>;


pub struct Providers {
    providers: HashMap<String, Provider>,
}

impl Providers {

    pub fn new() -> Providers {
        Providers {
            providers: HashMap::new(),
        }
    }

    pub fn add(&mut self, name: &str, provider: Provider) {
        self.providers.insert(name.to_string(), provider);
    }

    pub fn by_name(&self, name: &String) -> Option<Provider> {
        match self.providers.get(name) {
            Some(provider) => {
                Some(provider.clone())
            },
            None => None,
        }
    }

}


#[derive(Clone)]
pub struct Provider {
    validator_func: ValidatorFunc,
    env_func: EnvFunc,
}

impl Provider {

    pub fn new(validator: ValidatorFunc, env: EnvFunc) -> Provider {
        Provider {
            validator_func: validator,
            env_func: env,
        }
    }

    pub fn validate(&self, config: json::Json) -> bool {
        let validator = self.validator_func;
        validator(config)
    }

    pub fn env(&self, config: json::Json) -> HashMap<String, String> {
        let env = self.env_func;
        env(config)
    }

}


#[derive(Clone)]
pub struct HookProvider {
    provider: Provider,
    config: json::Json,
}

impl HookProvider {

    pub fn new(provider: Provider, config: json::Json) -> HookProvider {
        HookProvider {
            provider: provider,
            config: config,
        }
    }

    pub fn validate(&self) -> bool {
        self.provider.validate(self.config.clone())
    }

    pub fn env(&self) -> HashMap<String, String> {
        self.provider.env(self.config.clone())
    }

}
