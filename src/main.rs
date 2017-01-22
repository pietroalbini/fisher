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

// Optional support for compiling with clippy
#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

extern crate rustc_serialize;
extern crate regex;
extern crate ansi_term;
extern crate url;
extern crate rand;
extern crate tiny_http;
extern crate libc;
extern crate signal;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate clap;
#[cfg(feature = "provider-github")] extern crate ring;
#[cfg(test)] extern crate hyper;

#[macro_use] mod utils;
mod cli;
mod providers;
mod hooks;
mod errors;
mod processor;
mod jobs;
mod web;
mod app;
mod requests;
mod native;

use std::time::{Instant, Duration};

use libc::{SIGINT, SIGTERM};
use ansi_term::{Style, Colour};


fn main() {
    let signal_trap = signal::trap::Trap::trap(&[
        SIGINT,  // Interrupt the program
        SIGTERM,  // Interrupt the program
    ]);

    // Load the options from the CLI arguments
    let options = errors::unwrap(cli::parse());

    // Show the relevant options
    println!("{} {}",
        Style::new().bold().paint("Concurrent jobs:"),
        options.max_threads
    );
    println!("{} {}",
        Style::new().bold().paint("Health checks:  "),
        if options.enable_health { "enabled" } else { "disabled" }
    );
    println!("{} {}",
        Style::new().bold().paint("Proxy support:  "),
        if let Some(proxies) = options.behind_proxies {
            format!("enabled (behind {} proxies)", proxies)
        } else { "disabled".to_string() }
    );

    println!("");

    // Create a new Fisher instance
    let mut factory = app::AppFactory::new(&options);

    // Collect all the hooks from the directory
    let mut hooks = errors::unwrap(hooks::collect(&options.hooks_dir));
    println!("{} ({} total)",
        Style::new().bold().paint("Collected hooks:"), hooks.len(),
    );

    // Load all the hooks in the Fisher instance
    let hooks_names = {
        let mut names: Vec<String> = hooks.keys().cloned().collect();
        names.sort();
        names
    };
    for name in &hooks_names {
        factory.add_hook(name.clone(), hooks.remove(name).unwrap());
        println!("- {}", name);
    }

    // Start Fisher
    let app_result = factory.start();
    if let Err(error) = app_result {
        println!("{} on {}: {}",
            Colour::Red.bold().paint("Failed to start the Web API"),
            options.bind, error,
        );
        ::std::process::exit(1);
    }
    let mut app = app_result.unwrap();

    println!("{} on {}",
        Colour::Green.bold().paint("Web API listening"), app.web_address(),
    );

    // Wait for signals
    loop {
        match signal_trap.wait(Instant::now()) {
            Some(SIGINT) | Some(SIGTERM) => break,
            _ => {},
        }
        ::std::thread::sleep(Duration::new(0, 100));
    }

    // Stop Fisher
    app.stop();
}
