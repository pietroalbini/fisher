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
mod prelude;

mod status;
mod standalone;
#[cfg(feature = "provider-github")] mod github;
#[cfg(feature = "provider-gitlab")] mod gitlab;
#[cfg(test)] pub mod testing;

use errors::FisherResult;
pub use providers::core::{Factories, ProviderFactory, Provider, HookProvider};
pub use providers::core::BoxedProvider;


// This macro simplifies adding new providers
macro_rules! provider {
    ($factories:expr, $name:expr, $provider:path) => {{
        use $provider as provider;

        fn factory(config: &str) -> FisherResult<BoxedProvider> {
            let prov = try!(provider::new(config));
            Ok(Box::new(prov) as BoxedProvider)
        }

        $factories.add($name, factory);
    }};
    ($factories:expr, $name:expr, $provider:path, on $cfg:meta) => {{
        #[cfg($cfg)]
        fn inner(factories: &mut Factories) {
            provider!(factories, $name, $provider);
        }
        #[cfg(not($cfg))]
        fn inner(_factories: &mut Factories) {}

        inner(&mut $factories);
    }};
}


lazy_static! {
    static ref FACTORIES: Factories = {
        let mut f = Factories::new();

        provider!(f,
            "Standalone",
            self::standalone::StandaloneProvider
        );
        provider!(f,
            "Status",
            self::status::StatusProvider
        );
        provider!(f,
            "GitHub",
            self::github::GitHubProvider,
            on feature="provider-github"
        );
        provider!(f,
            "GitLab",
            self::gitlab::GitLabProvider,
            on feature="provider-gitlab"
        );

        // This is added only during unit tests
        provider!(f,
            "Testing",
            self::testing::TestingProvider,
            on test
        );

        f
    };
}


pub fn get(name: &str, config: &str) -> FisherResult<HookProvider> {
    let name = name.to_string();

    // Get the related factory
    let factory = try!(FACTORIES.by_name(&name));

    // Create a new provider
    let provider = try!(factory(&config));
    Ok(HookProvider::new(provider, name))
}
