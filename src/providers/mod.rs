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

mod core;
mod status;
mod standalone;
#[cfg(feature = "provider-github")] mod github;
#[cfg(feature = "provider-gitlab")] mod gitlab;
#[cfg(test)] pub mod testing;

use errors::FisherResult;
pub use providers::core::{Provider, HookProvider};


// This macro simplifies adding new providers
macro_rules! provider {
    ($providers:expr, $name:expr, $module:path) => {{
        use $module as module;
        $providers.add($name, Provider::new(
            $name.to_string(),
            module::check_config,
            module::request_type,
            module::validate,
            module::env,
        ));
    }};
    ($providers:expr, $name:expr, $module:path, $cfg:meta) => {{
        #[cfg($cfg)]
        fn inner(providers: &mut core::Providers) {
            provider!(providers, $name, $module);
        }
        #[cfg(not($cfg))]
        fn inner(_providers: &mut core::Providers) {}

        inner(&mut $providers);
    }};
}


lazy_static! {
    static ref PROVIDERS: core::Providers = {
        let mut p = core::Providers::new();

        provider!(p, "Standalone", self::standalone);
        provider!(p, "Status", self::status);
        provider!(p, "GitHub", self::github, feature="provider-github");
        provider!(p, "GitLab", self::gitlab, feature="provider-gitlab");

        // This is added only during unit tests
        provider!(p, "Testing", self::testing, test);

        p
    };
}


pub fn get(name: &str, raw_config: &str) -> FisherResult<HookProvider> {
    // Use an owned string
    let config = raw_config.to_string();

    // Get the associated provider
    let provider = try!(PROVIDERS.by_name(&name.to_string()));
    HookProvider::new(provider, config)
}
