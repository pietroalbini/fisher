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

extern crate clap;
extern crate ansi_term;
extern crate signal;
extern crate libc;
extern crate fisher;

use std::time::{Instant, Duration};

use clap::{App, Arg};
use libc::{SIGINT, SIGTERM};
use ansi_term::{Style, Colour};


pub fn parse_cli() -> fisher::Result<fisher::FisherOptions> {
    let matches = App::new("Fisher")
        .about("Simple webhooks catcher")
        .version(env!("CARGO_PKG_VERSION"))

        .arg(Arg::with_name("hooks").required(true).index(1)
             .value_name("DIR")
             .help("The directory which contains the hooks"))

        .arg(Arg::with_name("bind").takes_value(true)
             .long("bind").short("b")
             .value_name("PORT")
             .help("The port to bind fish to"))

        .arg(Arg::with_name("max_threads").takes_value(true)
             .long("jobs").short("j")
             .value_name("JOBS_COUNT")
             .help("How much concurrent jobs to run"))

        .arg(Arg::with_name("disable_health")
             .long("no-health")
             .help("Disable the /health endpoint"))

        .arg(Arg::with_name("behind_proxies").takes_value(true)
             .long("behind-proxies")
             .value_name("PROXIES_COUNT")
             .help("How much proxies are behind the app"))

        .get_matches();

    let max_threads = (
        matches.value_of("max_threads").unwrap_or("1").parse::<u16>()
    )?;

    let mut behind_proxies = None;
    if let Some(count) = matches.value_of("behind_proxies") {
        behind_proxies = Some(count.parse::<u8>()?);
    }

    Ok(fisher::FisherOptions {
        bind: matches.value_of("bind").unwrap_or("127.0.0.1:8000").to_string(),
        hooks_dir: matches.value_of("hooks").unwrap().to_string(),
        max_threads: max_threads,
        enable_health: ! matches.is_present("disable_health"),
        behind_proxies: behind_proxies,
    })
}


fn print_err(error: &fisher::Error) {
    println!("{} {}",
        ::ansi_term::Colour::Red.bold().paint("Error:"),
        error,
    );
    if let Some(location) = error.location() {
        println!("{} {}",
            ::ansi_term::Colour::Yellow.bold().paint("Location:"),
            location,
        );
    }
    if let Some(hook) = error.processing() {
        println!("{} {}",
            ::ansi_term::Colour::Yellow.bold().paint("While processing:"),
            hook,
        );
    }
}


fn app() -> fisher::Result<()> {
    let signal_trap = signal::trap::Trap::trap(&[
        SIGINT,  // Interrupt the program
        SIGTERM,  // Interrupt the program
    ]);

    // Load the options from the CLI arguments
    let options = parse_cli()?;

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
    let mut factory = fisher::Fisher::new(&options);

    // Listen for incoming logs
    factory.listen_logs(|event: &fisher::logger::LogEvent| {
        use fisher::logger::LogEvent::*;

        match *event {
            Error(ref err) => print_err(err),
        }
    });

    // Collect the hooks
    factory.collect_hooks(&options.hooks_dir)?;
    {
        let mut hook_names = factory.hook_names().collect::<Vec<&String>>();
        hook_names.sort();

        println!("{} ({} total)",
            Style::new().bold().paint("Collected hooks:"), hook_names.len(),
        );
        for name in &hook_names {
            println!("- {}", name);
        }
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
    let app = app_result.unwrap();

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
    app.stop()?;

    Ok(())
}


fn main() {
    ::std::process::exit(match app() {
        Ok(..) => 0,
        Err(error) => {
            print_err(&error);
            1
        },
    });
}
