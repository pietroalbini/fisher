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

use std::collections::HashMap;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::path::PathBuf;

use web::requests::Request;
use providers::{Provider, testing};
use utils;


pub fn dummy_request() -> Request {
    Request {
        headers: HashMap::new(),
        params: HashMap::new(),
        source: SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 80
        ),
        body: String::new(),
    }
}


pub fn testing_provider() -> Provider {
    Provider::new(
        "Testing".to_string(),
        testing::check_config,
        testing::request_type,
        testing::validate,
        testing::env,
    )
}


macro_rules! create_hook {
    ($tempdir:expr, $name:expr, $( $line:expr ),* ) => {
        use std::fs;
        use std::os::unix::fs::OpenOptionsExt;
        use std::io::Write;

        let mut hook_path = $tempdir.clone();
        hook_path.push($name);

        let mut hook = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .mode(0o755)
            .open(&hook_path)
            .unwrap();

        let res = write!(hook, "{}", concat!(
            $(
                $line, "\n",
            )*
        ));
        res.unwrap();
    };
}


pub fn sample_hooks() -> PathBuf {
    // Create a sample directory with some hooks
    let tempdir = utils::create_temp_dir().unwrap();

    create_hook!(tempdir, "example.sh",
        r#"#!/bin/bash"#,
        r#"## Fisher-Testing: {}"#,
        r#"echo "Hello world""#
    );

    tempdir
}
