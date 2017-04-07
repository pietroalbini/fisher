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

use jobs::Job;
use utils::Serial;


#[derive(Debug)]
pub struct ScheduledJob {
    job: Job,
    priority: isize,
    serial: Serial,
}

impl ScheduledJob {

    pub fn new(job: Job, priority: isize, serial: Serial) -> ScheduledJob {
        ScheduledJob {
            job: job,
            priority: priority,
            serial: serial,
        }
    }

    pub fn job(&self) -> &Job {
        &self.job
    }

    pub fn trigger_status_hooks(&self) -> bool {
        self.job.trigger_status_hooks()
    }
}

impl Ord for ScheduledJob {

    fn cmp(&self, other: &ScheduledJob) -> Ordering {
        let priority_ord = self.priority.cmp(&other.priority);

        if priority_ord == Ordering::Equal {
            self.serial.cmp(&other.serial).reverse()
        } else {
            priority_ord
        }
    }
}

impl PartialOrd for ScheduledJob {

    fn partial_cmp(&self, other: &ScheduledJob) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ScheduledJob {

    fn eq(&self, other: &ScheduledJob) -> bool {
        self.priority == other.priority
    }
}

impl Eq for ScheduledJob {}

