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

// Optional support for compiling with clippy
#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

extern crate regex;
extern crate ansi_term;
extern crate url;
extern crate rand;
extern crate tiny_http;
extern crate nix;
#[macro_use] extern crate serde_json;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate lazy_static;
#[cfg(feature = "provider-github")] extern crate ring;
#[cfg(test)] extern crate hyper;

#[macro_use] mod utils;
mod app;
mod hooks;
mod jobs;
mod native;
mod processor;
mod providers;
mod requests;
mod web;
pub mod common;

// Public API
pub use app::{Fisher, RunningFisher};
pub use common::errors::*;
