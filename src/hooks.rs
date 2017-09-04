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

use scripts::Script as Hook;


pub struct HooksCollector {
    dirs: VecDeque<ReadDir>,
    state: Arc<State>,
    base: PathBuf,
    recursive: bool,
}

impl HooksCollector {

    pub fn new<P: AsRef<Path>>(base: P, state: Arc<State>, recursive: bool)
                               -> Result<Self> {
        let mut dirs = VecDeque::new();
        dirs.push_front(read_dir(&base)?);

        Ok(HooksCollector {
            dirs: dirs,
            state: state,
            base: base.as_ref().to_path_buf(),
            recursive: recursive,
        })
    }

    fn collect_file(&mut self, e: PathBuf) -> Result<Option<Arc<Hook>>> {
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

        Ok(Some(Arc::new(Hook::load(name, exec, &self.state)?)))
    }
}

impl Iterator for HooksCollector {
    type Item = Result<Arc<Hook>>;

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
                            if let Some(hook) = result {
                                return Some(Ok(hook));
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
    use std::io::Write;
    use std::fs;
    use std::sync::Arc;

    use common::state::State;

    use utils;
    use common::prelude::*;

    use super::HooksCollector;


    #[test]
    fn test_collect() {
        let base = utils::create_temp_dir().unwrap();
        let state = Arc::new(State::new());

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
        let mut hooks = Vec::new();
        for hook in HooksCollector::new(&base, state.clone(), false).unwrap() {
            hooks.push(hook.unwrap().name().to_string());
        }

        // There should be only two collected hooks
        assert_eq!(hooks.len(), 2);
        assert!(hooks.contains(&"test-hook.sh".to_string()));
        assert!(hooks.contains(&"another-test.sh".to_string()));

        // Collect with recursion
        let mut hooks = Vec::new();
        for hook in HooksCollector::new(&base, state.clone(), true).unwrap() {
            hooks.push(hook.unwrap().name().to_string());
        }

        // There should be only two collected hooks
        assert_eq!(hooks.len(), 3);
        assert!(hooks.contains(&"test-hook.sh".to_string()));
        assert!(hooks.contains(&"another-test.sh".to_string()));
        assert!(hooks.contains(&"a-directory/hook-in-subdir.sh".to_string()));

        // Then add an hook with an invalid provider
        create_hook!(base, "invalid.sh",
            r#"#!/bin/bash"#,
            r#"## Fisher-InvalidHookDoNotUseThisNamePlease: invalid"#,
            r#"echo "hi";"#
        );

        // The collection should fail
        let mut error = None;
        for hook in HooksCollector::new(&base, state.clone(), false).unwrap() {
            if let Err(err) = hook {
                error = Some(err);
                break;
            }
        }
        let error = error.unwrap();

        if let ErrorKind::ProviderNotFound(ref name) = *error.kind() {
            assert_eq!(name, "InvalidHookDoNotUseThisNamePlease");
        } else {
            panic!("Wrong error kind: {:?}", error.kind());
        }

        fs::remove_dir_all(&base).unwrap();
    }
}
