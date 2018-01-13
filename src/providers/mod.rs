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
mod github;
mod gitlab;
#[cfg(test)]
pub mod testing;


pub mod prelude {
    pub use providers::ProviderTrait;
    pub use requests::{Request, RequestType};
    pub use common::prelude::*;
    pub use scripts::EnvBuilder;
}


pub use self::status::{StatusEvent, StatusEventKind, StatusProvider};


use requests::{Request, RequestType};
use common::prelude::*;
use scripts::EnvBuilder;


/// This trait should be implemented by every Fisher provider
/// The objects implementing this trait must also implement Clone and Debug
pub trait ProviderTrait: ::std::fmt::Debug {
    /// This method should create a new instance of the provider, from a
    /// given configuration string
    fn new(&str) -> Result<Self>
    where
        Self: Sized;

    /// This method should validate an incoming request, returning its
    /// type if the request is valid
    fn validate(&self, &Request) -> RequestType;

    /// This method should build the environment to process an incoming
    /// request
    fn build_env(&self, req: &Request, builder: &mut EnvBuilder) -> Result<()>;

    /// This method tells the scheduler if the hook should trigger status hooks
    /// after the request is processed. By default this returns true, change it
    /// only if you really know what you're doing
    fn trigger_status_hooks(&self, _req: &Request) -> bool {
        true
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

            pub fn new(name: &str, config: &str) -> Result<Provider> {
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
                match *self {
                    $(
                        #[cfg($cfg)]
                        Provider::$name(ref prov) => {
                            (prov as &ProviderTrait).validate(req)
                        },
                    )*
                }
            }

            pub fn build_env(
                &self, req: &Request, builder: &mut EnvBuilder,
            ) -> Result<()> {
                match *self {
                    $(
                        #[cfg($cfg)]
                        Provider::$name(ref prov) => {
                            (prov as &ProviderTrait).build_env(req, builder)
                        },
                    )*
                }
            }

            pub fn trigger_status_hooks(&self, req: &Request) -> bool {
                match *self {
                    $(
                        #[cfg($cfg)]
                        Provider::$name(ref prov) => {
                            (prov as &ProviderTrait).trigger_status_hooks(req)
                        }
                    )*
                }
            }

            #[allow(dead_code)]
            pub fn name(&self) -> &str {
                match *self {
                    $(
                        #[cfg($cfg)]
                        Provider::$name(..) => stringify!($name),
                    )*
                }
            }
        }
    };
}


ProviderEnum! {
    any(test, not(test)) | Standalone => self::standalone::StandaloneProvider,
    any(test, not(test)) | Status => self::status::StatusProvider,
    any(test, not(test)) | GitHub => self::github::GitHubProvider,
    any(test, not(test)) | GitLab => self::gitlab::GitLabProvider,
    test | Testing => self::testing::TestingProvider
}
