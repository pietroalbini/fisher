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

use std::io::Read;
use std::net::IpAddr;
use std::collections::HashMap;

use nickel;
use hyper::uri::RequestUri;
use url::form_urlencoded;
use rustc_serialize::json;

use jobs::JobOutput;


#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum RequestType {
    ExecuteHook,
    Ping,
    Internal,
}


#[derive(Clone)]
pub struct Request {
    pub source: IpAddr,
    pub headers: HashMap<String, String>,
    pub params: HashMap<String, String>,
    pub body: String,
}

impl From<JobOutput> for Request {

    fn from(output: JobOutput) -> Request {
        let mut params = HashMap::new();
        params.insert("hook_name".into(), output.hook_name.clone());
        params.insert(
            "event".into(),
            if output.success { "job_completed" } else { "job_failed" }.into()
        );

        Request {
            source: "127.0.0.1".parse().unwrap(),
            headers: HashMap::new(),
            params: params,
            body: json::encode(&output).unwrap(),
        }
    }
}


pub fn convert_request(req: &mut nickel::Request) -> Request {
    let source = req.origin.remote_addr.clone().ip();

    // Convert headers from the hyper representation to strings
    let mut headers = HashMap::new();
    for header in req.origin.headers.iter() {
        headers.insert(header.name().to_string(), header.value_string());
    }

    // Get the body
    let mut body = String::new();
    let _ = req.origin.read_to_string(&mut body);

    let params = params_from_request(req);

    Request {
        source: source,
        headers: headers,
        params: params,
        body: body,
    }
}


fn params_from_request(req: &nickel::Request) -> HashMap<String, String> {
    let ref uri = req.origin.uri;

    let query_string = match *uri {
        RequestUri::AbsoluteUri(ref url) => Some(url.query()),
        RequestUri::AbsolutePath(ref s) => Some(s.splitn(2, '?').nth(1)),
        _ => None,
    };

    match query_string {
        Some(path) => {
            // Don't do anything if there is no query string
            if path.is_none() {
                return HashMap::new();
            }
            let path = path.unwrap();

            params_from_query(path)
        },
        None => HashMap::new(),
    }
}


pub fn params_from_query(query: &str) -> HashMap<String, String> {
    let mut hashmap = HashMap::new();
    for (a, b) in form_urlencoded::parse(query.as_bytes()).into_owned() {
        hashmap.insert(a, b);
    }
    hashmap
}
