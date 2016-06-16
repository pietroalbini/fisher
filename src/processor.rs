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


pub struct Job<'h> {
    hook: &'h Hook,
}

impl<'h> Job<'h> {

    pub fn new(hook: &'h Hook) -> Job<'h> {
        Job {
            hook: hook,
        }
    }

    pub fn process(&self) {
        println!("Processing hook {}!", self.hook.name);
    }

}


pub struct ProcessorInstance<'h> {
    hooks: &'h HashMap<String, Hook>,
    jobs: VecDeque<Job<'h>>,
}

impl<'h> ProcessorInstance<'h> {

    pub fn new(hooks: &'h HashMap<String, Hook>) -> ProcessorInstance<'h> {
        ProcessorInstance {
            hooks: hooks,
            jobs: VecDeque::new(),
        }
    }

    pub fn schedule(&mut self, name: String) {
        let hook = self.hooks.get(&name).unwrap();
        self.jobs.push_back(Job::new(hook));
    }

    pub fn pop_job(&mut self) -> Option<Job<'h>> {
        self.jobs.pop_front()
    }

    pub fn jobs_present(&self) -> bool {
        ! self.jobs.is_empty()
    }

}
