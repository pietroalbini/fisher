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

use std::fs;
use std::path::Path;
use std::collections::HashMap;
use std::collections::hash_map::Keys as HashMapKeys;
use std::slice::Iter as SliceIter;
use std::os::unix::fs::PermissionsExt;
use std::io::{BufReader, BufRead};
use std::sync::Arc;

use regex::Regex;
use serde_json;

use providers::{Provider, StatusEventKind};
use requests::{Request, RequestType};
use errors::FisherResult;


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
}

impl Preferences {

    fn empty() -> Self {
        Preferences {
            priority: None,
        }
    }

    #[inline]
    fn priority(&self) -> isize {
        self.priority.unwrap_or(0)
    }
}


struct LoadHeadersOutput {
    preferences: Preferences,
    providers: Vec<Arc<Provider>>,
}


#[derive(Debug)]
pub struct Hook {
    name: String,
    exec: String,
    priority: isize,
    providers: Vec<Arc<Provider>>,
}

impl Hook {

    fn load(name: String, exec: String) -> FisherResult<Hook> {
        let headers = Hook::load_headers(&exec)?;

        Ok(Hook {
            name: name,
            exec: exec,
            priority: headers.preferences.priority(),
            providers: headers.providers,
        })
    }

    fn load_headers(file: &str) -> FisherResult<LoadHeadersOutput> {
        let f = fs::File::open(file).unwrap();
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
                    continue;  // Don't capture anything else for this line
                }
            }

            if let Some(cap) = PROVIDER_HEADER_RE.captures(&content) {
                let name = &cap[1];
                let data = &cap[2];

                match Provider::new(name, data) {
                    Ok(provider) => {
                        providers.push(Arc::new(provider));
                    },
                    Err(mut error) => {
                        error.set_file(file.into());
                        error.set_line(line_number);
                        return Err(error);
                    }
                }
            }
        }

        Ok(LoadHeadersOutput {
            preferences: if let Some(pref) = preferences { pref } else {
                Preferences::empty()
            },
            providers: providers,
        })
    }

    pub fn validate(&self, req: &Request)
                   -> (RequestType, Option<Arc<Provider>>) {
        if ! self.providers.is_empty() {
            // Check every provider if they're present
            for provider in &self.providers {
                return (provider.validate(req), Some(provider.clone()))
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


pub struct HookNamesIter<'a> {
    iter: HashMapKeys<'a, String, Arc<Hook>>,
}

impl<'a> Iterator for HookNamesIter<'a> {
    type Item = &'a String;

    fn next(&mut self) -> Option<&'a String> {
        self.iter.next()
    }
}


#[derive(Debug)]
pub struct HookProvider {
    pub hook: Arc<Hook>,
    pub provider: Arc<Provider>,
}


#[derive(Debug)]
pub struct Hooks {
    hooks: HashMap<String, Arc<Hook>>,
    status_hooks: HashMap<StatusEventKind, Vec<HookProvider>>,
}

impl Hooks {

    pub fn new() -> Self {
        Hooks {
            hooks: HashMap::new(),
            status_hooks: HashMap::new(),
        }
    }

    pub fn insert(&mut self, name: String, hook: Hook) {
        let hook = Arc::new(hook);
        self.hooks.insert(name, hook.clone());

        for provider in &hook.providers {
            if let Provider::Status(ref status) = *provider.as_ref() {
                // Load all the kinds of events
                for event in status.events() {
                    self.status_hooks.entry(*event)
                        .or_insert_with(Vec::new)
                        .push(HookProvider {
                            hook: hook.clone(),
                            provider: provider.clone(),
                        });
                }
            }
        }
    }

    pub fn get(&self, name: &str) -> Option<Arc<Hook>> {
        match self.hooks.get(name) {
            Some(hook) => Some(hook.clone()),
            None => None,
        }
    }

    pub fn names(&self) -> HookNamesIter {
        HookNamesIter {
            iter: self.hooks.keys()
        }
    }

    pub fn status_hooks_iter(&self, kind: StatusEventKind) -> SliceIter<HookProvider> {
        if let Some(hook_providers) = self.status_hooks.get(&kind) {
            hook_providers.iter()
        } else {
            // Return an empty iterator if there is no hook for this kind
            (&[]).iter()
        }
    }
}


pub fn collect<T: AsRef<Path>>(base: T)
        -> FisherResult<HashMap<String, Hook>> {
    let mut hooks = HashMap::new();

    for entry in fs::read_dir(&base)? {
        let pathbuf = entry?.path();
        let path = pathbuf.as_path();

        // Check if the file is actually a file
        if ! path.is_file() {
            continue;
        }

        // Check if the file is executable and readable
        let mode = path.metadata()?.permissions().mode();
        if ! ((mode & 0o111) != 0 && (mode & 0o444) != 0) {
            // Skip files with wrong permissions
            continue
        }

        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let exec = fs::canonicalize(path)?.to_str().unwrap().into();

        let hook = Hook::load(name.clone(), exec)?;
        hooks.insert(name, hook);
    }

    Ok(hooks)
}


#[cfg(test)]
mod tests {
    use std::os::unix::fs::OpenOptionsExt;
    use std::io::Write;
    use std::fs;

    use utils::testing::*;
    use utils;
    use errors::ErrorKind;
    use providers::StatusEventKind;

    use super::{Hook, Hooks, collect};


    macro_rules! assert_hook {
        ($base:expr, $name:expr) => {{
            // Get the hook path
            let mut path = $base.clone();
            path.push($name);
            let path_str = path.to_str().unwrap().to_string();

            let hook = Hook::load(
                $name.to_string(), path_str.clone()
            ).unwrap();

            assert_eq!(hook.name, $name.to_string());
            assert_eq!(hook.exec, path_str.clone());

            hook
        }};
    }


    #[test]
    fn test_hook_loading() {
        let base = sample_hooks();

        // Try to load a naked hook
        create_hook!(base, "naked.sh",
            r#"#!/bin/bash"#,
            r#"echo "Hello world"#
        );
        let hook = assert_hook!(base, "naked.sh");
        assert_eq!(hook.priority, 0);
        assert!(hook.providers.is_empty());

        // Try to load an hook with some preferences
        create_hook!(base, "preferences.sh",
            r#"#!/bin/bash"#,
            r#"## Fisher: {"priority": 5}"#,
            r#"echo "Hello world"#
        );
        let hook = assert_hook!(base, "preferences.sh");
        assert_eq!(hook.priority, 5);
        assert!(hook.providers.is_empty());

        // Try to load an hook with a provider
        create_hook!(base, "one-provider.sh",
            r#"#!/bin/bash"#,
            r#"## Fisher-Testing: {}"#,
            r#"echo "Hello world"#
        );
        let hook = assert_hook!(base, "one-provider.sh");
        assert_eq!(hook.priority, 0);
        assert_eq!(hook.providers.len(), 1);
        assert_eq!(
            hook.providers.get(0).unwrap().name(), "Testing".to_string()
        );

        // Try to load an hook with a provider and some preferences
        create_hook!(base, "preferences-provider.sh",
            r#"#!/bin/bash"#,
            r#"## Fisher: {"priority": 5}"#,
            r#"## Fisher-Testing: {}"#,
            r#"echo "Hello world"#
        );
        let hook = assert_hook!(base, "preferences-provider.sh");
        assert_eq!(hook.priority, 5);
        assert_eq!(hook.providers.len(), 1);
        assert_eq!(
            hook.providers.get(0).unwrap().name(), "Testing".to_string()
        );

        // Try to load an hook with two providers
        create_hook!(base, "two-providers.sh",
            r#"#!/bin/bash"#,
            r#"## Fisher-Standalone: {"secret": "abcde"}"#,
            r#"## Fisher-Testing: {}"#,
            r#"echo "Hello world"#
        );
        let hook = assert_hook!(base, "two-providers.sh");
        assert_eq!(hook.priority, 0);
        assert_eq!(hook.providers.len(), 2);
        assert_eq!(
            hook.providers.get(0).unwrap().name(), "Standalone".to_string()
        );
        assert_eq!(
            hook.providers.get(1).unwrap().name(), "Testing".to_string()
        );

        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn test_loading_providers() {
        macro_rules! load_providers {
            ($base:expr, $file:expr) => {{
                let mut path = $base.clone();
                path.push($file);

                Hook::load_headers(
                    &path.to_str().unwrap().to_string()
                ).map(|res| res.providers)
            }};
        };

        macro_rules! assert_provider {
            ($providers:expr, $index:expr, $name:expr) => {{
                let provider = $providers.get($index).unwrap();
                assert_eq!(provider.name(), $name);
            }};
        };

        let base = sample_hooks();

        // This hook is empty, it shouldn't return things
        create_hook!(base, "empty.sh", "");
        let providers = load_providers!(base, "empty.sh").unwrap();
        assert!(providers.is_empty());

        // This hook is not empty, but it doesn't contain any comment
        create_hook!(base, "no-comments.sh",
            r#"echo "hi";"#,
            r#"sleep 1;"#
        );
        let providers = load_providers!(base, "no-comments.sh").unwrap();
        assert!(providers.is_empty());

        // This hook contains only a shebang and some comments
        create_hook!(base, "comments.sh",
            r#"#!/bin/bash"#,
            r#"# Hey, that's a comment!"#,
            r#"echo "hi";"#
        );
        let providers = load_providers!(base, "comments.sh").unwrap();
        assert!(providers.is_empty());

        // This hook contains multiple simil-providers, but not a real one
        create_hook!(base, "simil.sh",
            r#"#!/bin/bash"#,
            r#"## Something-Testing: fisher"#,
            r#"## Fisher-Testing something"#,
            r#"echo "hi";"#
        );
        let providers = load_providers!(base, "simil.sh").unwrap();
        assert!(providers.is_empty());

        // This hook contains a single valid provider
        create_hook!(base, "single-provider.sh",
            r#"#!/bin/bash"#,
            r#"## Fisher-Testing: something"#,
            r#"## Something-Testing: fisher"#,
            r#"# hey!"#,
            r#"echo "hi";"#
        );
        let providers = load_providers!(base, "single-provider.sh").unwrap();
        assert_provider!(providers, 0, "Testing");

        // This hook contains multiple valid providers
        create_hook!(base, "two-providers.sh",
            r#"#!/bin/bash"#,
            r#"## Fisher-Testing: something"#,
            r#"## Fisher-Standalone: {"secret": "12345"}"#,
            r#"# hey!"#,
            r#"echo "hi";"#
        );
        let providers = load_providers!(base, "two-providers.sh").unwrap();
        assert_provider!(providers, 0, "Testing");
        assert_provider!(providers, 1, "Standalone");

        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn test_hooks_status_hooks_iter() {
        let base = utils::create_temp_dir().unwrap();

        // Create a standard hook
        create_hook!(base, "test.sh",
            r#"#!/bin/bash"#,
            r#"## Fisher-Testing: something"#,
            r#"echo "hi";"#
        );

        // Create two different status hooks
        create_hook!(base, "status1.sh",
            r#"#!/bin/bash"#,
            r#"## Fisher-Status: {"events": ["job_completed", "job_failed"]}"#,
            r#"echo "hi";"#
        );
        create_hook!(base, "status2.sh",
            r#"#!/bin/bash"#,
            r#"## Fisher-Status: {"events": ["job_failed"]}"#,
            r#"echo "hi";"#
        );

        let mut hooks = Hooks::new();
        hooks.insert("test.sh".into(), assert_hook!(base, "test.sh"));
        hooks.insert("status1.sh".into(), assert_hook!(base, "status1.sh"));
        hooks.insert("status2.sh".into(), assert_hook!(base, "status2.sh"));

        assert_eq!(
            hooks.status_hooks_iter(StatusEventKind::JobCompleted)
                 .map(|hp| hp.hook.name())
                 .collect::<Vec<&str>>(),
            vec!["status1.sh"]
        );
        assert_eq!(
            hooks.status_hooks_iter(StatusEventKind::JobFailed)
                 .map(|hp| hp.hook.name())
                 .collect::<Vec<&str>>(),
            vec!["status1.sh", "status2.sh"]
        );

        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn test_collect() {
        let base = utils::create_temp_dir().unwrap();

        // Create two valid hooks
        create_hook!(base, "test-hook.sh",
            r#"#!/bin/bash"#,
            r#"## Fisher-Testing: something"#,
            r#"echo "hi";"#
        );
        create_hook!(base, "another-test.sh",
            r#"#!/bin/bash"#,
            r#"## Fisher-Testing: something"#,
            r#"echo "bye";"#
        );

        // Create a directory
        let mut dir_path = base.clone();
        dir_path.push("a-directory");
        fs::create_dir(&dir_path).unwrap();;

        // Create an hook into that directory
        create_hook!(dir_path, "hook-in-subdir.sh",
            r#"#!/bin/bash"#,
            r#"## Fisher-Testing: something"#,
            r#"echo "I'm useless :(";"#
        );

        // Create a non-executable file
        let mut hook_path = base.clone();
        hook_path.push("non-executable.sh");
        let mut hook = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .mode(0o644)
            .open(&hook_path)
            .unwrap();
        let res = write!(hook, "{}", concat!(
            r#"#!/bin/bash"#,
            r#"## Fisher-Testing: something"#,
            r#"echo "I'm also useless :(("#
        ));
        res.unwrap();

        // Collect all the hooks in the base
        let hooks = collect(&base).unwrap();

        // There should be only two collected hooks
        assert_eq!(hooks.len(), 2);
        assert!(hooks.contains_key("test-hook.sh"));
        assert!(hooks.contains_key("another-test.sh"));

        // Then add an hook with an invalid provider
        create_hook!(base, "invalid.sh",
            r#"#!/bin/bash"#,
            r#"## Fisher-InvalidHookDoNotUseThisNamePlease: invalid"#,
            r#"echo "hi";"#
        );

        // The collection should fail
        let error = collect(&base).err().unwrap();
        if let ErrorKind::ProviderNotFound(ref name) = *error.kind() {
            assert_eq!(name, "InvalidHookDoNotUseThisNamePlease");
        } else {
            panic!("Wrong error kind: {:?}", error.kind());
        }

        fs::remove_dir_all(&base).unwrap();
    }
}
