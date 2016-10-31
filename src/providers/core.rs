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
use std::path::PathBuf;

use requests::{Request, RequestType};
use errors::{FisherResult, ErrorKind};


pub type BoxedProvider = Box<Provider + Sync + Send>;
pub type ProviderFactory = fn(&str) -> FisherResult<BoxedProvider>;


/// This trait should be implemented by every Fisher provider
/// The objects implementing this trait must also implement Clone
pub trait Provider: ProviderClone {

    /// This method should create a new instance of the provider, from a
    /// given configuration string
    fn new(&str) -> FisherResult<Self> where Self: Sized;

    /// This method should validate an incoming request, returning its
    /// type if the request is valid
    fn validate(&self, &Request) -> RequestType;

    /// This method should provide the environment variables of the provided
    /// request. Those variables will be passed to the process
    fn env(&self, &Request) -> HashMap<String, String>;

    /// This method should prepare the directory in which the hook will be run.
    /// This means, if you want to add extra files in there you should use
    /// this. You're not required to implement this method
    fn prepare_directory(&self, _req: &Request, _path: &PathBuf)
                         -> FisherResult<()> {
        Ok(())
    }
}


// This trick allows to clone Box<Provider>
// Thanks to DK. and Chris Morgan on StackOverflow:
// @ http://stackoverflow.com/a/30353928/2204144
pub trait ProviderClone {
    fn box_clone(&self) -> BoxedProvider;
}

impl<T> ProviderClone for T where T: 'static + Provider + Sync + Send + Clone {

    fn box_clone(&self) -> BoxedProvider {
        Box::new(self.clone())
    }
}

impl Clone for BoxedProvider {

    fn clone(&self) -> BoxedProvider {
        self.box_clone()
    }
}


pub struct Factories {
    factories: HashMap<String, ProviderFactory>,
}

impl Factories {

    pub fn new() -> Factories {
        Factories {
            factories: HashMap::new(),
        }
    }

    pub fn add(&mut self, name: &str, factory: ProviderFactory) {
        self.factories.insert(name.to_string(), factory);
    }

    pub fn by_name(&self, name: &str) -> FisherResult<&ProviderFactory> {
        match self.factories.get(&name.to_string()) {
            Some(ref factory) => {
                Ok(factory)
            },
            None => {
                let kind = ErrorKind::ProviderNotFound(name.to_string());
                Err(kind.into())
            },
        }
    }
}


#[derive(Clone)]
pub struct HookProvider {
    provider: BoxedProvider,
    name: String,
}

impl HookProvider {

    pub fn new(provider: BoxedProvider, name: String) -> HookProvider {
        HookProvider {
            provider: provider,
            name: name,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn validate(&self, req: &Request) -> RequestType {
        self.provider.validate(req)
    }

    pub fn env(&self, req: &Request) -> HashMap<String, String> {
        self.provider.env(req)
    }

    pub fn prepare_directory(&self, req: &Request, path: &PathBuf)
                             -> FisherResult<()> {
        self.provider.prepare_directory(req, path)
    }
}


#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;

    use utils::testing::*;
    use utils;
    use requests::RequestType;

    use super::Factories;

    #[test]
    fn test_factories() {
        // Create a new instance of Factories
        let mut factories = Factories::new();

        // Add a dummy factory
        factories.add("Sample", testing_provider_factory());

        // You should be able to get a factory if it exists
        assert!(factories.by_name(&"Sample".to_string()).is_ok());

        // But if it doesn't exists you should get an error
        assert!(factories.by_name(&"Not-Exists".to_string()).is_err());
    }

    #[test]
    fn test_hook_provider() {
        let provider_factory = testing_provider_factory();
        let request = dummy_request();

        // Try to create an hook provider with an invalid config
        assert!(provider_factory("FAIL").is_err());

        // Create an hook provider with a valid config
        let provider_res = provider_factory("yes");
        assert!(provider_res.is_ok());

        let provider = provider_res.unwrap();

        // You should be able to call the request validator
        assert_eq!(provider.validate(&request), RequestType::ExecuteHook);

        // You should be able to call the environment creator
        assert_eq!(provider.env(&request), HashMap::new());

        // You should be able to call the directory preparator
        let directory = utils::create_temp_dir().unwrap();
        provider.prepare_directory(&request, &directory).unwrap();
        fs::remove_dir_all(&directory).unwrap();
    }
}
