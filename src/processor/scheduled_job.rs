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

use std::cmp::Ordering;

use common::prelude::*;
use common::serial::Serial;

use super::types::{Job, JobContext, JobOutput, ScriptId};


#[derive(Debug)]
pub struct ScheduledJob<S: ScriptsRepositoryTrait> {
    job: Job<S>,
    priority: isize,
    serial: Serial,
}

impl<S: ScriptsRepositoryTrait> ScheduledJob<S> {
    pub fn new(job: Job<S>, priority: isize, serial: Serial) -> Self {
        ScheduledJob {
            job: job,
            priority: priority,
            serial: serial,
        }
    }

    pub fn execute(&self, ctx: &JobContext<S>) -> Result<JobOutput<S>> {
        let mut result = self.job.execute(ctx);

        // Ensure the right error location is set
        if let &mut Err(ref mut error) = &mut result {
            error.set_location(
                ErrorLocation::HookProcessing(self.hook_name().into()),
            );
        }

        result
    }

    pub fn hook_id(&self) -> ScriptId<S> {
        self.job.script_id()
    }

    pub fn hook_name(&self) -> &str {
        self.job.script_name()
    }
}

impl<S: ScriptsRepositoryTrait> Ord for ScheduledJob<S> {
    fn cmp(&self, other: &ScheduledJob<S>) -> Ordering {
        let priority_ord = self.priority.cmp(&other.priority);

        if priority_ord == Ordering::Equal {
            self.serial.cmp(&other.serial).reverse()
        } else {
            priority_ord
        }
    }
}

impl<S: ScriptsRepositoryTrait> PartialOrd for ScheduledJob<S> {
    fn partial_cmp(&self, other: &ScheduledJob<S>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<S: ScriptsRepositoryTrait> PartialEq for ScheduledJob<S> {
    fn eq(&self, other: &ScheduledJob<S>) -> bool {
        self.priority == other.priority
    }
}

impl<S: ScriptsRepositoryTrait> Eq for ScheduledJob<S> {}
