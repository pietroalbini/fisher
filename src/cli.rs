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

use clap::{App, Arg};

use errors::FisherResult;
use app::FisherOptions;


fn create_cli<'a, 'b>() -> App<'a, 'b> {
    let app = App::new("Fisher")
        .about("Simple webhooks catcher")
        .version(crate_version!())

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
    ;

    app
}


pub fn parse() -> FisherResult<FisherOptions> {
    let matches = create_cli().get_matches();

    let max_threads = try!(
        matches.value_of("max_threads").unwrap_or("1").parse::<u16>()
    );

    let mut behind_proxies = None;
    if let Some(count) = matches.value_of("behind_proxies") {
        behind_proxies = Some(try!(count.parse::<u8>()));
    }

    Ok(FisherOptions {
        bind: matches.value_of("bind").unwrap_or("127.0.0.1:8000").to_string(),
        hooks_dir: matches.value_of("hooks").unwrap().to_string(),
        max_threads: max_threads,
        enable_health: ! matches.is_present("disable_health"),
        behind_proxies: behind_proxies,
    })
}
