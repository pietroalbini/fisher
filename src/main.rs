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

extern crate rustc_serialize;
extern crate regex;
#[macro_use] extern crate nickel;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate clap;

mod cli;
mod providers;
mod hooks;
mod errors;
mod processor;
mod web;

use std::process;
use std::thread;


fn main() {
    let options = cli::parse();

    let collected_hooks = hooks::collect(&options.hooks_dir);
    if collected_hooks.is_err() {
        println!("Error: {}", collected_hooks.err().unwrap());
        process::exit(1);
    }
    let hooks = collected_hooks.unwrap();

    let mut processor = processor::ProcessorInstance::new(&hooks);

    // Start the web application
    let webapp = web::create_app();
    let options_clone = options.clone();
    thread::spawn(move || {
        let bind: &str = &options_clone.bind;
        webapp.listen(bind);
    });
    println!("Listening on {}", options.bind);

    loop {}
}
