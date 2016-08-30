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
mod standalone;

use errors::FisherResult;
use providers::core::Provider;
pub use providers::core::HookProvider;


// This macro simplifies adding new providers
macro_rules! provider {
    ($providers:expr, $name:expr, $module:path) => {
        use $module as module;
        $providers.add($name, Provider::new(
            module::check_config,
            module::validate,
            module::env,
        ));
    };
}


lazy_static! {
    static ref PROVIDERS: core::Providers = {
        let mut p = core::Providers::new();

        provider!(p, "Standalone", self::standalone);

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
