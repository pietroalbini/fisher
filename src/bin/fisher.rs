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

extern crate ansi_term;
extern crate clap;
extern crate fisher;
extern crate libc;
extern crate signal;

use std::time::{Duration, Instant};

use clap::{App, Arg};
use libc::{SIGUSR1, SIGINT, SIGTERM};
use ansi_term::{Colour, Style};


struct CliArgs {
    hooks_dir: String,
    recursive: bool,
    bind: String,
    env: Vec<String>,
    max_threads: u16,
    behind_proxies: u8,
    enable_health: bool,
}


fn parse_cli() -> fisher::Result<CliArgs> {
    let matches = App::new("Fisher")
        .about("Simple webhooks catcher")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::with_name("hooks")
                .required(true)
                .index(1)
                .value_name("DIR")
                .help("The directory which contains the hooks"),
        )
        .arg(
            Arg::with_name("recursive")
                .long("recursive")
                .short("r")
                .help("Search for hooks recursively"),
        )
        .arg(
            Arg::with_name("bind")
                .takes_value(true)
                .long("bind")
                .short("b")
                .value_name("PORT")
                .help("The port to bind fish to"),
        )
        .arg(
            Arg::with_name("env")
                .takes_value(true)
                .multiple(true)
                .long("env")
                .short("e")
                .value_name("KEY=VALUE")
                .help("Add additional environment variables"),
        )
        .arg(
            Arg::with_name("max_threads")
                .takes_value(true)
                .long("jobs")
                .short("j")
                .value_name("JOBS_COUNT")
                .help("How much concurrent jobs to run"),
        )
        .arg(
            Arg::with_name("disable_health")
                .long("no-health")
                .help("Disable the /health endpoint"),
        )
        .arg(
            Arg::with_name("behind_proxies")
                .takes_value(true)
                .long("behind-proxies")
                .value_name("PROXIES_COUNT")
                .help("How much proxies are behind the app"),
        )
        .get_matches();

    Ok(CliArgs {
        hooks_dir: matches.value_of("hooks").unwrap().into(),
        recursive: matches.is_present("recursive"),
        bind: matches.value_of("bind").unwrap_or("127.0.0.1:8000").into(),
        env: {
            if let Some(values) = matches.values_of("env") {
                values.map(|v| v.to_string()).collect()
            } else {
                Vec::new()
            }
        },
        max_threads: {
            matches
                .value_of("max_threads")
                .unwrap_or("1")
                .parse::<u16>()?
        },
        behind_proxies: {
            if let Some(count) = matches.value_of("behind_proxies") {
                count.parse::<u8>()?
            } else {
                0
            }
        },
        enable_health: !matches.is_present("disable_health"),
    })
}


fn print_err<T>(result: fisher::Result<T>) -> fisher::Result<T> {
    // Show a nice error message
    if let Err(ref error) = result {
        error.pretty_print();
    }

    result
}


fn app() -> fisher::Result<()> {
    let signal_trap = signal::trap::Trap::trap(&[
        SIGINT,  // Interrupt the program
        SIGTERM, // Interrupt the program
        SIGUSR1, // Reload Fisher
    ]);

    // Load the options from the CLI arguments
    let args = parse_cli()?;

    // Show the relevant options
    println!(
        "{} {}",
        Style::new().bold().paint("Concurrent jobs:"),
        args.max_threads
    );
    println!(
        "{} {}",
        Style::new().bold().paint("Health checks:  "),
        if args.enable_health {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!(
        "{} {}",
        Style::new().bold().paint("Proxy support:  "),
        if args.behind_proxies != 0 {
            format!("enabled (behind {} proxies)", args.behind_proxies)
        } else {
            "disabled".to_string()
        }
    );

    println!("");

    // Create a new Fisher instance
    let mut factory = fisher::Fisher::new();

    factory.max_threads = args.max_threads;
    factory.behind_proxies = args.behind_proxies;
    factory.bind = &args.bind;
    factory.enable_health = args.enable_health;

    factory.collect_scripts(args.hooks_dir, args.recursive)?;
    {
        let mut hook_names = factory.script_names().collect::<Vec<String>>();
        hook_names.sort();

        println!(
            "{} ({} total)",
            Style::new().bold().paint("Collected hooks:"),
            hook_names.len(),
        );
        for name in &hook_names {
            println!("- {}", name);
        }
    }

    // Set the extra environment variables
    for env in &args.env {
        factory.raw_env(env)?;
    }

    // Start Fisher
    let app_result = factory.start();
    if let Err(error) = app_result {
        println!(
            "{} on {}: {}",
            Colour::Red.bold().paint("Failed to start the Web API"),
            args.bind,
            error,
        );
        ::std::process::exit(1);
    }
    let mut app = app_result.unwrap();

    println!(
        "{} on {}",
        Colour::Green.bold().paint("Web API listening"),
        app.web_address(),
    );

    // Wait for signals
    loop {
        match signal_trap.wait(Instant::now()) {
            Some(SIGINT) | Some(SIGTERM) => break,
            Some(SIGUSR1) => {
                println!(
                    "{} hooks list",
                    Colour::Green.bold().paint("Reloading")
                );

                // Don't crash if the reload fails, just show errors
                // No changes are applied if the reload fails
                let _ = print_err(app.reload());
            }
            _ => {}
        }
        ::std::thread::sleep(Duration::new(0, 100));
    }

    // Stop Fisher
    app.stop()?;

    Ok(())
}


fn main() {
    ::std::process::exit(match print_err(app()) {
        Ok(..) => 0,
        Err(..) => 1,
    });
}
