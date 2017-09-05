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

use std::fs::{read_dir, canonicalize, ReadDir};
use std::path::{Path, PathBuf};
use std::collections::VecDeque;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use common::prelude::*;
use common::state::State;

use scripts::Script as Script;


pub(in scripts) struct Collector {
    dirs: VecDeque<ReadDir>,
    state: Arc<State>,
    base: PathBuf,
    recursive: bool,
}

impl Collector {

    pub(in scripts) fn new<P: AsRef<Path>>(
        base: P, state: Arc<State>, recursive: bool,
    ) -> Result<Self> {
        let mut dirs = VecDeque::new();
        dirs.push_front(read_dir(&base)?);

        Ok(Collector {
            dirs: dirs,
            state: state,
            base: base.as_ref().to_path_buf(),
            recursive: recursive,
        })
    }

    fn collect_file(&mut self, e: PathBuf) -> Result<Option<Arc<Script>>> {
        if e.is_dir() {
            if self.recursive {
                self.dirs.push_back(read_dir(&e)?);
            }
            return Ok(None);
        }

        // Check if the file is executable and readable
        let mode = e.metadata()?.permissions().mode();
        if ! ((mode & 0o111) != 0 && (mode & 0o444) != 0) {
            // Skip files with wrong permissions
            return Ok(None);
        }

        // Try to remove the prefix from the path
        let name = match e.strip_prefix(&self.base) {
            Ok(stripped) => stripped,
            Err(_) => &e,
        }.to_str().unwrap().to_string();

        let exec = canonicalize(&e)?.to_str().unwrap().into();

        Ok(Some(Arc::new(Script::load(name, exec, &self.state)?)))
    }
}

impl Iterator for Collector {
    type Item = Result<Arc<Script>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let entry = if let Some(iter) = self.dirs.get_mut(0) {
                iter.next()
            } else {
                // No more directories to search in
                return None;
            };

            match entry {
                // Found an entry
                Some(Ok(entry)) => {
                    match self.collect_file(entry.path()) {
                        Ok(result) => {
                            if let Some(script) = result {
                                return Some(Ok(script));
                            }
                            // If None is returned get another one
                        },
                        Err(err) => {
                            return Some(Err(err));
                        },
                    }
                },
                // I/O error while getting the next entry
                Some(Err(err)) => {
                    return Some(Err(err.into()));
                },
                // No more entries in the directory
                None => {
                    // Don't search in this directory anymore
                    let _ = self.dirs.pop_front();
                },
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use std::os::unix::fs::OpenOptionsExt;
    use std::fs;

    use common::prelude::*;
    use scripts::test_utils::*;

    use super::Collector;


    fn assert_collected(
        env: &TestEnv, recurse: bool, expected: &[&str]
    ) -> Result<()> {
        let mut found = 0;

        let c = Collector::new(&env.scripts_dir(), env.state(), recurse)?;
        for script in c {
            found += 1;

            let script = script?;
            if ! expected.contains(&script.name()) {
                panic!("Unexpected script collected: {}", script.name());
            }
        }

        assert_eq!(found, expected.len());
        Ok(())
    }


    #[test]
    fn test_scripts_collection_collects_all_the_valid_scripts() {
        test_wrapper(|env| {
            // Create two scripts in the top level
            env.create_script("first.sh", &[])?;
            env.create_script("second.sh", &[])?;

            // Create a non-executable script
            fs::OpenOptions::new()
                .create(true)
                .write(true)
                .mode(0o644)
                .open(env.scripts_dir().join("third.sh"))?;

            // Create a directory with another script
            let dir = env.scripts_dir().join("subdir");
            fs::create_dir(&dir)?;
            env.create_script_into(&dir, "fourth.sh", &[])?;

            // Ensure the collected scripts are the right ones
            assert_collected(&env, false, &["first.sh", "second.sh"])?;
            assert_collected(&env, true, &[
                "first.sh", "second.sh", "subdir/fourth.sh",
            ])?;

            Ok(())
        });
    }


    #[test]
    fn test_scripts_collection_with_invalid_scripts_fails() {
        test_wrapper(|env| {
            // Create a valid script
            env.create_script("valid.sh", &[
                r#"#!/bin/bash"#,
                r#"## Fisher-Testing: {}"#,
                r#"echo "I'm valid!""#,
            ])?;

            // Ensure the scripts collection succedes
            assert_collected(&env, false, &["valid.sh"])?;

            // Create an additional invalid script
            env.create_script("invalid.sh", &[
                r#"#!/bin/bash"#,
                r#"## Fisher-InvalidProviderDoNotReallyCreateThis: {}"#,
                r#"echo "I'm not valid :(""#,
            ])?;

            // Ensure the scripts collection fails
            let err = assert_collected(&env, false, &[
                "valid.sh", "invalid.sh",
            ]).err().expect("The collection should return an error");

            // Ensure the returned error is correct
            if let ErrorKind::ProviderNotFound(ref name) = *err.kind() {
                assert_eq!(name, "InvalidProviderDoNotReallyCreateThis");
            } else {
                panic!("Wrong kind of error returned");
            }

            Ok(())
        })
    }
}
