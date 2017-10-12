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

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use common::prelude::*;
use common::state::{State, UniqueId};
use providers::{Provider, StatusEvent, StatusEventKind};
use requests::Request;
use scripts::collector::Collector;
use scripts::jobs::{Job, JobOutput};
use scripts::script::{Script, ScriptProvider};


pub struct ScriptsIter {
    inner: Arc<RwLock<RepositoryInner>>,
    count: usize,
}

impl ScriptsIter {
    fn new(inner: Arc<RwLock<RepositoryInner>>) -> Self {
        ScriptsIter { inner, count: 0 }
    }
}

impl Iterator for ScriptsIter {
    type Item = Arc<Script>;

    fn next(&mut self) -> Option<Self::Item> {
        self.count += 1;

        match self.inner.read() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }.scripts
            .get(self.count - 1)
            .cloned()
    }
}


pub struct ScriptNamesIter {
    iter: ScriptsIter,
}

impl ScriptNamesIter {
    fn new(iter: ScriptsIter) -> Self {
        ScriptNamesIter { iter: iter }
    }
}

impl Iterator for ScriptNamesIter {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|script| script.name().to_string())
    }
}


pub struct StatusJobsIter {
    inner: Arc<RwLock<RepositoryInner>>,
    event: StatusEvent,
    count: usize,
}

impl StatusJobsIter {
    fn new(inner: Arc<RwLock<RepositoryInner>>, event: StatusEvent) -> Self {
        StatusJobsIter {
            inner,
            event,
            count: 0,
        }
    }
}

impl Iterator for StatusJobsIter {
    type Item = Job;

    fn next(&mut self) -> Option<Self::Item> {
        self.count += 1;

        let inner = match self.inner.read() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        if let Some(all) = inner.status_hooks.get(&self.event.kind()) {
            if let Some(hp) = all.get(self.count - 1).cloned() {
                Some(Job::new(
                    hp.script,
                    Some(hp.provider),
                    Request::Status(self.event.clone()),
                ))
            } else {
                None
            }
        } else {
            None
        }
    }
}


#[derive(Debug)]
struct RepositoryInner {
    scripts: Vec<Arc<Script>>,
    by_id: HashMap<UniqueId, Arc<Script>>,
    by_name: HashMap<String, Arc<Script>>,
    status_hooks: HashMap<StatusEventKind, Vec<ScriptProvider>>,
}

impl RepositoryInner {
    pub fn new() -> Self {
        RepositoryInner {
            scripts: Vec::new(),
            by_id: HashMap::new(),
            by_name: HashMap::new(),
            status_hooks: HashMap::new(),
        }
    }

    pub fn insert(&mut self, script: Arc<Script>) {
        self.scripts.push(script.clone());
        self.by_id.insert(script.id(), script.clone());
        self.by_name
            .insert(script.name().to_string(), script.clone());

        for provider in &script.providers {
            if let Provider::Status(ref status) = *provider.as_ref() {
                // Load all the kinds of events
                for event in status.events() {
                    self.status_hooks
                        .entry(*event)
                        .or_insert_with(Vec::new)
                        .push(ScriptProvider {
                            script: script.clone(),
                            provider: provider.clone(),
                        });
                }
            }
        }
    }

    pub fn get_by_name(&self, name: &str) -> Option<Arc<Script>> {
        self.by_name.get(name).cloned()
    }
}


#[derive(Debug)]
pub struct Repository {
    inner: Arc<RwLock<RepositoryInner>>,
}

impl Repository {
    pub fn get_by_name(&self, name: &str) -> Option<Arc<Script>> {
        match self.inner.read() {
            Ok(inner) => inner.get_by_name(name),
            Err(poisoned) => poisoned.get_ref().get_by_name(name),
        }
    }

    pub fn names(&self) -> ScriptNamesIter {
        ScriptNamesIter::new(self.iter())
    }
}

impl ScriptsRepositoryTrait for Repository {
    type Script = Script;
    type Job = Job;
    type ScriptsIter = ScriptsIter;
    type JobsIter = StatusJobsIter;

    fn id_exists(&self, id: &UniqueId) -> bool {
        match self.inner.read() {
            Ok(inner) => inner.by_id.contains_key(id),
            Err(poisoned) => poisoned.get_ref().by_id.contains_key(id),
        }
    }

    fn iter(&self) -> ScriptsIter {
        ScriptsIter::new(self.inner.clone())
    }

    fn jobs_after_output(&self, output: JobOutput) -> Option<StatusJobsIter> {
        if !output.trigger_status_hooks {
            return None;
        }

        let event = if output.success {
            StatusEvent::JobCompleted(output)
        } else {
            StatusEvent::JobFailed(output)
        };

        Some(StatusJobsIter::new(self.inner.clone(), event))
    }
}


#[derive(Debug)]
pub struct Blueprint {
    added: Vec<Arc<Script>>,
    collect_paths: Vec<(PathBuf, bool)>,

    inner: Arc<RwLock<RepositoryInner>>,
    state: Arc<State>,
}

impl Blueprint {
    pub fn new(state: Arc<State>) -> Self {
        Blueprint {
            added: Vec::new(),
            collect_paths: Vec::new(),

            inner: Arc::new(RwLock::new(RepositoryInner::new())),
            state: state,
        }
    }

    pub fn insert(&mut self, script: Arc<Script>) -> Result<()> {
        self.added.push(script);

        self.reload()?;
        Ok(())
    }

    pub fn collect_path<P: AsRef<Path>>(
        &mut self,
        path: P,
        recursive: bool,
    ) -> Result<()> {
        self.collect_paths
            .push((path.as_ref().to_path_buf(), recursive));

        self.reload()?;
        Ok(())
    }

    pub fn reload(&mut self) -> Result<()> {
        let mut inner = RepositoryInner::new();

        // Add manually added scripts
        for script in &self.added {
            inner.insert(script.clone());
        }

        // Collect scripts from paths
        let mut collector;
        for &(ref p, recursive) in &self.collect_paths {
            collector = Collector::new(p, self.state.clone(), recursive)?;
            for script in collector {
                inner.insert(script?);
            }
        }

        {
            let mut to_update = self.inner.write()?;
            *to_update = inner;
        }

        Ok(())
    }

    pub fn repository(&self) -> Repository {
        Repository {
            inner: self.inner.clone(),
        }
    }
}


#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use common::prelude::*;
    use providers::StatusEventKind;
    use scripts::test_utils::*;

    use super::{Blueprint, Repository};


    #[test]
    fn test_blueprint_allows_adding_scripts() {
        test_wrapper(|env| {
            // Create a script
            env.create_script(
                "first.sh",
                &[r#"#!/bin/bash"#, r#"echo "First script""#],
            )?;

            // Create a directory with two scripts
            let dir = env.tempdir()?;
            env.create_script_into(
                &dir,
                "second.sh",
                &[r#"#!/bin/bash"#, r#"echo "Second script""#],
            )?;
            env.create_script_into(
                &dir,
                "third.sh",
                &[r#"#!/bin/bash"#, r#"echo "Third script""#],
            )?;

            // Create a new empty blueprint
            let mut blueprint = Blueprint::new(env.state());

            // Add the single script to the blueprint
            blueprint.insert(Arc::new(env.load_script("first.sh")?))?;

            // Collect the directory with the blueprint
            blueprint.collect_path(&dir, false)?;

            // Ensure all the scripts are in the repository
            let repository = blueprint.repository();
            for script in &["first.sh", "second.sh", "third.sh"] {
                assert!(repository.get_by_name(script).is_some());
            }

            Ok(())
        });
    }


    #[test]
    fn test_blueprint_changes_are_applies_to_existing_repositories() {
        test_wrapper(|env| {
            // Create two scripts
            env.create_script(
                "first.sh",
                &[r#"#!/bin/bash"#, r#"echo "First script""#],
            )?;
            env.create_script(
                "second.sh",
                &[r#"#!/bin/bash"#, r#"echo "Second script""#],
            )?;

            // Create a new empty blueprint
            let mut blueprint = Blueprint::new(env.state());

            // Add one of the script to the blueprint
            blueprint.insert(Arc::new(env.load_script("first.sh")?))?;

            // Get the repository related to the blueprint
            let repository = blueprint.repository();

            // Ensure only the first script is present in the repository
            assert!(repository.get_by_name("first.sh").is_some());
            assert!(repository.get_by_name("second.sh").is_none());

            // Add another script to the blueprint
            blueprint.insert(Arc::new(env.load_script("second.sh")?))?;

            // Ensure all the scripts are present in the existing repository
            assert!(repository.get_by_name("first.sh").is_some());
            assert!(repository.get_by_name("second.sh").is_some());

            Ok(())
        });
    }


    #[test]
    fn test_blueprint_can_be_reloaded() {
        test_wrapper(|env| {
            // Create two scripts
            env.create_script(
                "first.sh",
                &[r#"#!/bin/bash"#, r#"echo "I'm the first script""#],
            )?;
            env.create_script(
                "second.sh",
                &[r#"#!/bin/bash"#, r#"echo "I'm the second script""#],
            )?;

            // Create a new blueprint and collect the directory
            let mut blueprint = Blueprint::new(env.state());
            blueprint.collect_path(&env.scripts_dir(), false)?;

            // Ensure the two scripts are present
            let repository = blueprint.repository();
            let id_original = repository
                .get_by_name("first.sh")
                .expect("The first.sh script was not collected")
                .id();
            assert!(repository.get_by_name("second.sh").is_some());
            assert!(repository.get_by_name("third.sh").is_none());

            // Create a new script and delete one of the existing ones
            env.create_script(
                "third.sh",
                &[r#"#!/bin/bash"#, r#"echo "I'm the third script""#],
            )?;
            fs::remove_file(env.scripts_dir().join("second.sh"))?;

            // Reload the blueprint
            blueprint.reload()?;

            // Ensure the correct scripts are present
            let id_new = repository
                .get_by_name("first.sh")
                .expect("The first.sh script was not collected")
                .id();
            assert!(repository.get_by_name("second.sh").is_none());
            assert!(repository.get_by_name("third.sh").is_some());

            // Ensure the script IDs are different
            assert_ne!(id_original, id_new);

            Ok(())
        });
    }

    #[test]
    fn test_no_changes_applied_if_blueprint_reload_fails() {
        test_wrapper(|env| {
            // Create a new script
            env.create_script(
                "first.sh",
                &[r#"#!/bin/bash"#, r#"echo "I'm the first script""#],
            )?;

            // Create a new script in another directory
            let dir = env.tempdir()?;
            env.create_script_into(
                &dir,
                "second.sh",
                &[r#"#!/bin/bash"#, r#"echo "I'm the second script""#],
            )?;

            // Create a new blueprint and collect the directories
            let mut blueprint = Blueprint::new(env.state());
            blueprint.collect_path(env.scripts_dir(), false)?;
            blueprint.collect_path(&dir, false)?;

            // Ensure the scripts are present
            let repository = blueprint.repository();
            assert!(repository.get_by_name("first.sh").is_some());
            assert!(repository.get_by_name("second.sh").is_some());
            assert!(repository.get_by_name("third.sh").is_none());

            // Remove the second directory and create a script in the other
            fs::remove_dir_all(&dir)?;
            env.create_script(
                "third.sh",
                &[r#"#!/bin/bash"#, r#"echo "I'm the third script""#],
            )?;

            // Reload the blueprint, and ensure it fails
            assert!(blueprint.reload().is_err());

            // Ensure no changes were applied
            assert!(repository.get_by_name("first.sh").is_some());
            assert!(repository.get_by_name("second.sh").is_some());
            assert!(repository.get_by_name("third.sh").is_none());

            Ok(())
        });
    }


    #[test]
    fn test_status_hooks_are_correctly_stored() {
        // Check in the internal data structure
        fn assert_status_hooks(
            repo: &Repository,
            kind: StatusEventKind,
            expect: &[&str],
        ) {
            let inner = repo.inner.read().unwrap();

            let mut count = 0;
            for script in inner.status_hooks.get(&kind).unwrap() {
                assert!(expect.contains(&script.script.name()));
                count += 1;
            }

            assert_eq!(expect.len(), count);
        }

        test_wrapper(|env| {
            // Create a script and two status hooks
            env.create_script(
                "normal.sh",
                &[
                    r#"#!/bin/bash"#,
                    r#"## Fisher-Testing: {}"#,
                    r#"echo "I'm just a normal script""#,
                ],
            )?;
            env.create_script(
                "status-both.sh",
                &[
                    r#"#!/bin/bash"#,
                    r#"## Fisher-Status: {"events": ["job_completed", "job_failed"]}"#,
                    r#"echo "I'm a status script!""#,
                ],
            )?;
            env.create_script(
                "status-failed.sh",
                &[
                    r#"#!/bin/bash"#,
                    r#"## Fisher-Status: {"events": ["job_failed"]}"#,
                    r#"echo "I'm a failure!""#,
                ],
            )?;

            // Create a new blueprint
            let mut blueprint = Blueprint::new(env.state());
            blueprint.collect_path(&env.scripts_dir(), false)?;

            // Ensure all the scripts are present
            let repository = blueprint.repository();
            assert!(repository.get_by_name("normal.sh").is_some());
            assert!(repository.get_by_name("status-both.sh").is_some());
            assert!(repository.get_by_name("status-failed.sh").is_some());

            // Ensure the correct status hooks are returned
            assert_status_hooks(
                &repository,
                StatusEventKind::JobCompleted,
                &["status-both.sh"],
            );
            assert_status_hooks(
                &repository,
                StatusEventKind::JobFailed,
                &["status-both.sh", "status-failed.sh"],
            );

            Ok(())
        })
    }
}
