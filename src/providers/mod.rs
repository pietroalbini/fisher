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

mod status;
mod standalone;
#[cfg(feature = "provider-github")] mod github;
#[cfg(feature = "provider-gitlab")] mod gitlab;
#[cfg(test)] pub mod testing;


pub mod prelude {

    pub use std::collections::HashMap;
    pub use std::path::PathBuf;

    pub use providers::ProviderTrait;
    pub use requests::{Request, RequestType};
    pub use errors::FisherResult;
}


pub use self::status::{StatusEvent, StatusEventKind};


use std::collections::HashMap;
use std::path::PathBuf;

use requests::{Request, RequestType};
use errors::{FisherResult, ErrorKind};


/// This trait should be implemented by every Fisher provider
/// The objects implementing this trait must also implement Clone and Debug
pub trait ProviderTrait: ::std::fmt::Debug {

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


macro_rules! ProviderEnum {
    ($($cfg:meta | $name:ident => $provider:path),*) => {

        #[derive(Debug)]
        pub enum Provider {
            $(
                #[cfg($cfg)]
                $name($provider),
            )*
        }

        impl Provider {

            pub fn new(name: &str, config: &str) -> FisherResult<Provider> {
                match name {
                    $(
                        #[cfg($cfg)]
                        stringify!($name) => {
                            use $provider as InnerProvider;
                            match InnerProvider::new(config) {
                                Ok(prov) => Ok(Provider::$name(prov)),
                                Err(err) => Err(err),
                            }
                        },
                    )*
                    _ => Err(
                        ErrorKind::ProviderNotFound(name.to_string()).into()
                    ),
                }
            }

            pub fn validate(&self, req: &Request) -> RequestType {
                match self {
                    $(
                        #[cfg($cfg)]
                        &Provider::$name(ref prov) => {
                            (prov as &ProviderTrait).validate(req)
                        },
                    )*
                }
            }

            pub fn env(&self, req: &Request) -> HashMap<String, String> {
                match self {
                    $(
                        #[cfg($cfg)]
                        &Provider::$name(ref prov) => {
                            (prov as &ProviderTrait).env(req)
                        },
                    )*
                }
            }

            pub fn prepare_directory(&self, req: &Request, path: &PathBuf)
                                    -> FisherResult<()> {
                match self {
                    $(
                        #[cfg($cfg)]
                        &Provider::$name(ref prov) => {
                            (prov as &ProviderTrait)
                                .prepare_directory(req, path)
                        },
                    )*
                }
            }

            pub fn name(&self) -> &str {
                match self {
                    $(
                        #[cfg($cfg)]
                        &Provider::$name(..) => stringify!($name),
                    )*
                }
            }
        }
    };
}


ProviderEnum! {
    any(test, not(test)) | Standalone => self::standalone::StandaloneProvider,
    any(test, not(test)) | Status => self::status::StatusProvider,
    feature="provider-github" | GitHub => self::github::GitHubProvider,
    feature="provider-gitlab" | GitLab => self::gitlab::GitLabProvider,
    test | Testing => self::testing::TestingProvider
}
