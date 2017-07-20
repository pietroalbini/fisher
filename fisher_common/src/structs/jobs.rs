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

//! Structs related to jobs used by Fisher.

use std::net::IpAddr;


/// Represents how a process exited.

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ProcessExit {
    /// The job exited normally returning the contained exit code.
    ExitCode(i32),

    /// The job was killed by a signal.
    Signal(i32),
}

impl ProcessExit {

    /// Checks if the process exited successfully
    pub fn success(&self) -> bool {
        match *self {
            ProcessExit::ExitCode(code) => code == 0,
            ProcessExit::Signal(_) => false,
        }
    }
}


/// This struct contains some details about a job.
#[derive(Debug, Clone)]
pub struct JobDetails {
    /// The name of the script.
    pub script_name: String,

    /// The IP that started the job.
    pub ip: IpAddr,

    /// If the job should trigger status hooks.
    pub trigger_status_hooks: bool,
}


/// This struct represents the output of a job.

#[derive(Debug, Clone)]
pub struct JobOutput {
    /// The standard output of the job.
    pub stdout: String,

    /// The standard error of the job.
    pub stderr: String,

    /// How the job exited.
    pub exit: ProcessExit,

    /// More details about the job.
    pub job: JobDetails,
}
