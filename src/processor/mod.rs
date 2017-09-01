// Copyright (C) 2017 Pietro Albini
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

//! This crate contains the processor used by the Fisher application, and its
//! public API.

#![warn(missing_docs)]

mod api;
mod scheduled_job;
mod scheduler;
mod thread;
mod types;
#[cfg(test)] mod test_utils;

pub use processor::api::{Processor, ProcessorApi};
