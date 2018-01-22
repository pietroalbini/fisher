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

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::Arc;

use regex::Regex;
use serde_json;

use common::prelude::*;
use common::state::{IdKind, State, UniqueId};

use providers::Provider;
use requests::{Request, RequestType};


#[derive(Debug, Clone)]
pub struct ScriptProvider {
    pub script: Arc<Script>,
    pub provider: Arc<Provider>,
}


lazy_static! {
    static ref PREFERENCES_HEADER_RE: Regex = Regex::new(
        r"## Fisher: (.*)"
    ).unwrap();
    static ref PROVIDER_HEADER_RE: Regex = Regex::new(
        r"## Fisher-([a-zA-Z]+): (.*)"
    ).unwrap();
}


#[derive(Debug, Deserialize)]
struct Preferences {
    priority: Option<isize>,
    parallel: Option<bool>,
}

impl Preferences {
    fn empty() -> Self {
        Preferences {
            priority: None,
            parallel: None,
        }
    }

    #[inline]
    fn priority(&self) -> isize {
        self.priority.unwrap_or(0)
    }

    #[inline]
    fn parallel(&self) -> bool {
        self.parallel.unwrap_or(true)
    }
}


struct LoadHeadersOutput {
    preferences: Preferences,
    providers: Vec<Arc<Provider>>,
}


fn load_headers(file: &str) -> Result<LoadHeadersOutput> {
    let f = File::open(file).unwrap();
    let reader = BufReader::new(f);

    let mut content;
    let mut line_number: u32 = 0;
    let mut providers = vec![];
    let mut preferences = None;
    for line in reader.lines() {
        line_number += 1;
        content = line.unwrap();

        // Just ignore everything after an empty line
        if content == "" {
            break;
        }

        if preferences.is_none() {
            if let Some(cap) = PREFERENCES_HEADER_RE.captures(&content) {
                preferences = Some(serde_json::from_str(&cap[1])?);
                continue; // Don't capture anything else for this line
            }
        }

        if let Some(cap) = PROVIDER_HEADER_RE.captures(&content) {
            let name = &cap[1];
            let data = &cap[2];

            match Provider::new(name, data) {
                Ok(provider) => {
                    providers.push(Arc::new(provider));
                }
                Err(mut error) => {
                    Err(error.chain_err(|| ErrorKind::ScriptParsingError(
                        file.into(), line_number,
                    )))?;
                }
            }
        }
    }

    Ok(LoadHeadersOutput {
        preferences: if let Some(pref) = preferences {
            pref
        } else {
            Preferences::empty()
        },
        providers: providers,
    })
}


#[derive(Debug)]
pub struct Script {
    id: UniqueId,
    name: String,
    exec: String,
    priority: isize,
    parallel: bool,
    pub(crate) providers: Vec<Arc<Provider>>,
}

impl Script {
    pub fn load(
        name: String,
        exec: String,
        state: &Arc<State>,
    ) -> Result<Self> {
        let headers = load_headers(&exec)?;

        Ok(Script {
            id: state.next_id(IdKind::HookId),
            name: name,
            exec: exec,
            priority: headers.preferences.priority(),
            parallel: headers.preferences.parallel(),
            providers: headers.providers,
        })
    }

    pub fn validate(
        &self,
        req: &Request,
    ) -> (RequestType, Option<Arc<Provider>>) {
        if !self.providers.is_empty() {
            // Check every provider if they're present
            for provider in &self.providers {
                let result = provider.validate(req);

                if result != RequestType::Invalid {
                    return (result, Some(provider.clone()));
                }
            }
            (RequestType::Invalid, None)
        } else {
            (RequestType::ExecuteHook, None)
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn exec(&self) -> &str {
        &self.exec
    }

    pub fn priority(&self) -> isize {
        self.priority
    }
}

impl ScriptTrait for Script {
    type Id = UniqueId;

    fn id(&self) -> UniqueId {
        self.id
    }

    fn can_be_parallel(&self) -> bool {
        self.parallel
    }
}


#[cfg(test)]
mod tests {
    use common::prelude::*;
    use requests::{Request, RequestType};
    use scripts::test_utils::*;


    #[test]
    fn test_scripts_are_loaded_properly() {
        // This little helper avoids repeating code all the time
        fn create_and_assert(
            env: &TestEnv,
            name: &str,
            content: &[&str],
            priority: isize,
            parallel: bool,
            providers: &[&str],
        ) -> Result<()> {
            // Create and load the script
            env.create_script(name, content)?;
            let script = env.load_script(name)?;

            // Check if the basic attributes are loaded properly
            assert_eq!(script.name(), name);
            assert_eq!(script.priority(), priority);
            assert_eq!(script.can_be_parallel(), parallel);

            // Check if the executable path is correct
            assert_eq!(
                script.exec(),
                env.scripts_dir().join(name).to_str().unwrap()
            );

            assert_eq!(script.providers.len(), providers.len());
            for provider in &script.providers {
                assert!(providers.contains(&provider.name()));
            }

            Ok(())
        }

        test_wrapper(|env| {
            // Check if naked scripts are loaded properly
            create_and_assert(
                &env,
                "naked.sh",
                &[r#"#!/bin/bash"#, r#"echo "This is a naked script""#],
                0,
                true,
                &[],
            )?;

            // Check if scripts with preferences are loaded properly
            create_and_assert(
                &env,
                "prefs.sh",
                &[
                    r#"#!/bin/bash"#,
                    r#"## Fisher: {"parallel": false, "priority": 5}"#,
                    r#"echo "This script has preferences""#,
                ],
                5,
                false,
                &[],
            )?;

            // Check if scripts with one provider are loaded properly
            create_and_assert(
                &env,
                "one-provider.sh",
                &[
                    r#"#!/bin/bash"#,
                    r#"## Fisher-Testing: {}"#,
                    r#"echo "This script has one provider""#,
                ],
                0,
                true,
                &["Testing"],
            )?;

            // Check if scripts with preferences and providers are loaded properly
            create_and_assert(
                &env,
                "provider-prefs.sh",
                &[
                    r#"#!/bin/bash"#,
                    r#"## Fisher: {"parallel": false, "priority": 5}"#,
                    r#"## Fisher-Testing: {}"#,
                    r#"echo "This script has one provider and some preferences""#,
                ],
                5,
                false,
                &["Testing"],
            )?;

            // Check if scripts with two providers are loaded properly
            create_and_assert(
                &env,
                "two-providers.sh",
                &[
                    r#"#!/bin/bash"#,
                    r#"## Fisher-Testing: {}"#,
                    r#"## Fisher-Standalone: {"secret": "abcde"}"#,
                    r#"echo "This script has one provider""#,
                ],
                0,
                true,
                &["Testing", "Standalone"],
            )?;

            Ok(())
        });
    }


    #[test]
    fn test_requests_can_be_validated_against_scripts() {
        test_wrapper(|env| {
            // Create all the needed scripts
            env.create_script(
                "single.sh",
                &[r#"#!/bin/bash"#, r#"## Fisher-Testing: {}"#, r#"echo "ok""#],
            )?;
            env.create_script(
                "failing.sh",
                &[
                    r#"#!/bin/bash"#,
                    r#"## Fisher-Standalone: {"secret": "abcde"}"#,
                    r#"echo "ok""#,
                ],
            )?;
            env.create_script(
                "multiple1.sh",
                &[
                    r#"#!/bin/bash"#,
                    r#"## Fisher-Testing: {}"#,
                    r#"## Fisher-Standalone: {"secret": "abcde"}"#,
                    r#"echo "ok""#,
                ],
            )?;
            env.create_script(
                "multiple2.sh",
                &[
                    r#"#!/bin/bash"#,
                    r#"## Fisher-Standalone: {"secret": "abcde"}"#,
                    r#"## Fisher-Testing: {}"#,
                    r#"echo "ok""#,
                ],
            )?;

            // Load all the needed scripts
            let single = env.load_script("single.sh")?;
            let failing = env.load_script("failing.sh")?;
            let multiple1 = env.load_script("multiple1.sh")?;
            let multiple2 = env.load_script("multiple2.sh")?;

            // Create a dummy web request
            let req = Request::Web(dummy_web_request());

            // Validate the request against the scripts
            assert!(single.validate(&req).0 == RequestType::ExecuteHook);
            assert!(failing.validate(&req).0 == RequestType::Invalid);
            assert!(multiple1.validate(&req).0 == RequestType::ExecuteHook);
            assert!(multiple2.validate(&req).0 == RequestType::ExecuteHook);

            Ok(())
        });
    }


    #[test]
    fn test_script_ids_are_unique() {
        test_wrapper(|env| {
            // Create two different scripts
            env.create_script(
                "script1.sh",
                &[r#"#!/bin/bash"#, r#"echo "Script 1""#],
            )?;
            env.create_script(
                "script2.sh",
                &[r#"#!/bin/bash"#, r#"echo "Script 2""#],
            )?;

            // Load the scripts three time
            let id1 = env.load_script("script1.sh")?.id();
            let id2 = env.load_script("script1.sh")?.id();
            let id3 = env.load_script("script2.sh")?.id();

            // Check all the IDs are different
            assert_ne!(id1, id2);
            assert_ne!(id1, id3);
            assert_ne!(id2, id1);
            assert_ne!(id2, id3);
            assert_ne!(id3, id1);
            assert_ne!(id3, id2);

            Ok(())
        });
    }
}
