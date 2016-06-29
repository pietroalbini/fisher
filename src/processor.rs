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

use std::collections::{HashMap, VecDeque};

use hooks::Hook;
use errors::FisherError;


pub struct Job {
    hook_name: String,
}

impl Job {

    pub fn new(hook_name: String) -> Job {
        Job {
            hook_name: hook_name,
        }
    }

    pub fn process(&self, hooks: &HashMap<String, Hook>) {
        let hook = hooks.get(&self.hook_name).unwrap();

        println!("Processing hook {}!", hook.name);
    }

}


pub struct ProcessorInstance<'a> {
    hooks: &'a HashMap<String, Hook>,
    jobs: VecDeque<Job>,
}

impl<'a> ProcessorInstance<'a> {

    pub fn new(hooks: &'a HashMap<String, Hook>) -> ProcessorInstance<'a> {
        ProcessorInstance {
            hooks: hooks,
            jobs: VecDeque::new(),
        }
    }

    pub fn schedule(&mut self, name: String) -> Result<(), FisherError> {
        if ! self.hooks.contains_key(&name) {
            return Err(FisherError::HookNotFound(name.clone()));
        }

        self.jobs.push_back(Job::new(name.clone()));
        Ok(())
    }

    pub fn pop_job(&mut self) -> Option<Job> {
        self.jobs.pop_front()
    }

}
