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

use rustc_serialize::json;

use errors::FisherResult;


#[derive(RustcDecodable)]
struct Config {
    secret: String,
    allowed_ips: Option<Vec<String>>,
}


pub fn check_config(input: String) -> FisherResult<()> {
    try!(json::decode::<Config>(&input));

    Ok(())
}


pub fn validate(_config: String) -> bool {
    true
}


pub fn env(_config: String) -> HashMap<String, String> {
    HashMap::new()
}
