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

#![recursion_limit="256"]

extern crate ansi_term;
#[macro_use]
extern crate error_chain;
#[cfg(test)]
extern crate hyper;
#[macro_use]
extern crate lazy_static;
extern crate nix;
extern crate rand;
extern crate regex;
extern crate ring;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate tempdir;
extern crate tiny_http;
extern crate url;
extern crate users;

#[macro_use]
mod utils;
mod app;
mod processor;
mod providers;
mod requests;
mod scripts;
mod web;
pub mod common;

// Public API
pub use app::Fisher;
pub use common::config::Config;
pub use common::errors::*;
