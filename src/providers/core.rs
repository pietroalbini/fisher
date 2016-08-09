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

use errors::{FisherResult, FisherError};


pub type CheckConfigFunc = fn(String) -> FisherResult<()>;
pub type ValidatorFunc = fn(String) -> bool;
pub type EnvFunc = fn(String) -> HashMap<String, String>;


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

    pub fn by_name(&self, name: &String) -> FisherResult<Provider> {
        match self.providers.get(name) {
            Some(provider) => {
                Ok(provider.clone())
            },
            None => {
                Err(FisherError::ProviderNotFound(name.clone()))
            },
        }
    }

}


#[derive(Clone)]
pub struct Provider {
    check_config_func: CheckConfigFunc,
    validator_func: ValidatorFunc,
    env_func: EnvFunc,
}

impl Provider {

    pub fn new(check_config: CheckConfigFunc, validator: ValidatorFunc,
               env: EnvFunc) -> Provider {
        Provider {
            check_config_func: check_config,
            validator_func: validator,
            env_func: env,
        }
    }

    pub fn check_config(&self, config: String) -> FisherResult<()> {
        let check_config = self.check_config_func;
        check_config(config)
    }

    pub fn validate(&self, config: String) -> bool {
        let validator = self.validator_func;
        validator(config)
    }

    pub fn env(&self, config: String) -> HashMap<String, String> {
        let env = self.env_func;
        env(config)
    }

}


#[derive(Clone)]
pub struct HookProvider {
    provider: Provider,
    config: String,
}

impl HookProvider {

    pub fn new(provider: Provider, config: String)
               -> FisherResult<HookProvider> {
        // First of all, check if the config is correct
        try!(provider.check_config(config.clone()));

        // Then return the new provider
        Ok(HookProvider {
            provider: provider,
            config: config,
        })
    }

    pub fn validate(&self) -> bool {
        self.provider.validate(self.config.clone())
    }

    pub fn env(&self) -> HashMap<String, String> {
        self.provider.env(self.config.clone())
    }

}
