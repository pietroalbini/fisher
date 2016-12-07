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
extern crate ansi_term;
extern crate chan_signal;
extern crate url;
extern crate rand;
extern crate tiny_http;
#[macro_use] extern crate chan;
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

use chan_signal::Signal;
use ansi_term::{Style, Colour};


fn main() {
    let exit_signal = chan_signal::notify(&[Signal::INT, Signal::TERM]);

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
    let hooks = errors::unwrap(hooks::collect(&options.hooks_dir));
    println!("{} ({} total)",
        Style::new().bold().paint("Collected hooks:"), hooks.len(),
    );

    // Load all the hooks in the Fisher instance
    let mut hooks_names: Vec<&String> = hooks.keys().collect();
    hooks_names.sort();
    for name in &hooks_names {
        factory.add_hook(&name, hooks.get(*name).unwrap().clone());
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

    // Wait until SIGINT or SIGTERM is received
    exit_signal.recv().unwrap();

    // Stop Fisher
    app.stop();
}
