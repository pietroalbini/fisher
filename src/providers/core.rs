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

use processor::Request;
use errors::{FisherResult, FisherError, ErrorKind};
use utils::CopyToClone;


pub type CheckConfigFunc = fn(&str) -> FisherResult<()>;
pub type ValidatorFunc = fn(&Request, &str) -> bool;
pub type EnvFunc = fn(&Request, &str) -> HashMap<String, String>;


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

    pub fn by_name(&self, name: &str) -> FisherResult<Provider> {
        match self.providers.get(&name.to_string()) {
            Some(provider) => {
                Ok(provider.clone())
            },
            None => {
                let kind = ErrorKind::ProviderNotFound(name.to_string());
                Err(FisherError::new(kind))
            },
        }
    }

}


#[derive(Clone)]
pub struct Provider {
    check_config_func: CopyToClone<CheckConfigFunc>,
    validator_func: CopyToClone<ValidatorFunc>,
    env_func: CopyToClone<EnvFunc>,
}

impl Provider {

    pub fn new(check_config: CheckConfigFunc, validator: ValidatorFunc,
               env: EnvFunc) -> Provider {
        Provider {
            check_config_func: CopyToClone::new(check_config),
            validator_func: CopyToClone::new(validator),
            env_func: CopyToClone::new(env),
        }
    }

    pub fn check_config(&self, config: &str) -> FisherResult<()> {
        // The func must be dereferenced, since it's wrapped in CopyToClone
        (*self.check_config_func)(config)
    }

    pub fn validate(&self, req: &Request, config: &str) -> bool {
        // The func must be dereferenced, since it's wrapped in CopyToClone
        (*self.validator_func)(req, config)
    }

    pub fn env(&self, req: &Request, config: &str)
               -> HashMap<String, String> {
        // The func must be dereferenced, since it's wrapped in CopyToClone
        (*self.env_func)(req, config)
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
        try!(provider.check_config(&config));

        // Then return the new provider
        Ok(HookProvider {
            provider: provider,
            config: config,
        })
    }

    pub fn validate(&self, req: &Request) -> bool {
        self.provider.validate(req, &self.config)
    }

    pub fn env(&self, req: &Request) -> HashMap<String, String> {
        self.provider.env(req, &self.config)
    }

}


#[cfg(test)]
pub mod tests {
    use std::str::FromStr;
    use std::collections::HashMap;
    use std::net::{IpAddr, SocketAddr};

    use processor::Request;
    use super::{Providers, Provider, HookProvider};

    #[test]
    fn test_providers() {
        // Create a new instance of Providers
        let mut providers = Providers::new();

        // Add a dummy provider
        providers.add("Sample", Provider::new(
            sample_provider::check_config,
            sample_provider::validate,
            sample_provider::env,
        ));

        // You should be able to get a provider if it exists
        assert!(providers.by_name(&"Sample".to_string()).is_ok());

        // But if it doesn't exists you should get an error
        assert!(providers.by_name(&"Not-Exists".to_string()).is_err());
    }

    #[test]
    fn test_provider() {
        // Create a dummy provider
        let provider = Provider::new(
            sample_provider::check_config,
            sample_provider::validate,
            sample_provider::env,
        );

        let request = dummy_request();

        // You should be able to call the configuration checker
        assert!(provider.check_config("yes").is_ok());

        // You should be able to call the request validator
        assert!(provider.validate(&request, "yes"));

        // You should be able to call the environment creator
        assert!(provider.env(&request, "") == HashMap::new());
    }

    #[test]
    fn test_hook_provider() {
        // Create a dummy provider
        let provider = Provider::new(
            sample_provider::check_config,
            sample_provider::validate,
            sample_provider::env,
        );

        let request = dummy_request();

        // Try to ceate an hook provider with an invalid config
        let provider_res = HookProvider::new(provider.clone(), "no".to_string());
        assert!(provider_res.is_err());

        // Create an hook provider with a valid config
        let provider_res = HookProvider::new(provider.clone(), "yes".to_string());
        assert!(provider_res.is_ok());

        let provider = provider_res.unwrap();

        // You should be able to call the request validator
        assert!(provider.validate(&request));

        // You should be able to call the environment creator
        assert!(provider.env(&request) == HashMap::new());
    }


    pub fn dummy_request() -> Request {
        Request {
            headers: HashMap::new(),
            params: HashMap::new(),
            source: SocketAddr::new(
                IpAddr::from_str("127.0.0.1").unwrap(), 80
            ),
        }
    }


    // This module contains all the functions for a sample provider
    mod sample_provider {
        use std::collections::HashMap;

        use errors::{FisherResult, FisherError, ErrorKind};
        use processor::Request;

        pub fn check_config(config: &str) -> FisherResult<()> {
            // If the configuration is "yes", then it's correct
            if config == "yes" {
                Ok(())
            } else {
                // This error doesn't make any sense, but it's still an error
                Err(FisherError::new(
                    ErrorKind::ProviderNotFound(String::new())
                ))
            }
        }

        pub fn validate(_req: &Request, _config: &str) -> bool {
            true
        }

        pub fn env(_req: &Request, _config: &str) -> HashMap<String, String> {
            HashMap::new()
        }
    }
}
