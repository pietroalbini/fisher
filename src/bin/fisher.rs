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

extern crate fisher;
extern crate nix;
extern crate toml;

use std::fs;
use std::io::Read;
use std::path::Path;

use fisher::*;
use nix::sys::signal::{Signal, SigSet};


static VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");


fn show_version() {
    if let Some(version) = VERSION {
        println!("Fisher {}", version);
    } else {
        println!("Fisher (version unknown)");
    }
}


fn usage(exit_code: i32, error_msg: &str) -> ! {
    if error_msg.len() > 0 {
        println!("Error: {}\n", error_msg);
    }
    println!("Usage: fisher <config_file>");
    println!("Execute `fisher --help` for more details");
    ::std::process::exit(exit_code);
}


fn parse_cli() -> String {
    // Parse the CLI args
    let mut only_args = false;
    let mut flag_help = false;
    let mut flag_version = false;
    let mut config_path = None;

    for arg in ::std::env::args().skip(1) {
        if !only_args && arg.chars().next() == Some('-') {
            match arg.as_str() {
                "--" => only_args = true,
                "-h" | "--help" => flag_help = true,
                "--version" => flag_version = true,
                _ => usage(1, &format!("invalid flag: {}", arg)),
            }
        } else if config_path.is_none() {
            config_path = Some(arg);
        } else {
            usage(1, &format!("unexpected argument: {}", arg));
        }
    }

    if flag_help {
        show_version();
        println!("Simple webhooks catcher\n");

        println!("ARGUMENTS");
        println!("  config_path   The path to the configuration file");
        println!();

        println!("OPTIONS");
        println!("  -h | --help   Show this message");
        println!("  --version     Show the Fisher version");

        ::std::process::exit(0);
    } else if flag_version {
        show_version();
        ::std::process::exit(0);
    } else if let Some(path) = config_path {
        path
    } else {
        usage(1, "too few arguments");
    }
}


fn read_config<P: AsRef<Path>>(path: P) -> Result<Config> {
    // Read the configuration from a file
    let mut file = fs::File::open(path)?;
    let mut buffer = String::new();
    file.read_to_string(&mut buffer)?;

    Ok(toml::from_str(&buffer).map_err(|e| {
        Error::from_kind(ErrorKind::BoxedError(Box::new(e)).into())
    })?)
}


fn app() -> Result<()> {
    // Capture only the signals Fisher uses
    let mut signals = SigSet::empty();
    signals.add(Signal::SIGINT);
    signals.add(Signal::SIGTERM);
    signals.add(Signal::SIGUSR1);
    signals.thread_block()?;

    let config_path = parse_cli();

    let mut app = Fisher::new(read_config(&config_path)?)?;
    println!("HTTP server listening on {}", app.web_address().unwrap());

    // Wait for signals while the other threads execute the application
    loop {
        match signals.wait()? {
            Signal::SIGINT | Signal::SIGTERM => break,
            Signal::SIGUSR1 => {
                println!("Reloading configuration and scripts...");

                // Don't crash if the reload fails, just show errors
                // No changes are applied if the reload fails
                match read_config(&config_path) {
                    Ok(new_config) => {
                        if let Err(err) = app.reload(new_config) {
                            err.pretty_print()
                        }
                    }
                    Err(err) => err.pretty_print(),
                }
            }
            _ => {}
        }
    }

    // Stop Fisher
    app.stop()?;

    Ok(())
}


fn main() {
    if let Err(err) = app() {
        err.pretty_print();
        std::process::exit(1);
    }
}
