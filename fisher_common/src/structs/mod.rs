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

//! Structs used by Fisher.

pub mod jobs;
pub mod requests;


/// This struct contains some information about how the processor is feeling.

#[derive(Copy, Clone, Debug, Serialize)]
pub struct HealthDetails {
    /// The number of jobs in the queue, waiting to be processed.
    pub queued_jobs: usize,

    /// The number of threads currently processing some jobs.
    pub busy_threads: u16,

    /// The total number of threads running, either waiting or working.
    pub max_threads: u16,
}
