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

use std::net::IpAddr;
use std::collections::HashMap;

use tiny_http;
use url::form_urlencoded;
use rustc_serialize::json;

use jobs::JobOutput;


#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum RequestType {
    ExecuteHook,
    Ping,
    Internal,
    Invalid,
}

impl RequestType {

    pub fn valid(&self) -> bool {
        match *self {
            RequestType::Invalid => false,
            _ => true,
        }
    }
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
            source: output.request_ip.parse().unwrap(),
            headers: HashMap::new(),
            params: params,
            body: json::encode(&output).unwrap(),
        }
    }
}

impl<'a> From<&'a mut tiny_http::Request> for Request {

    fn from(origin: &'a mut tiny_http::Request) -> Request {
        // Get the source IP
        let source = origin.remote_addr().ip().clone();

        // Get the headers
        let mut headers = HashMap::new();
        for header in origin.headers() {
            headers.insert(
                header.field.as_str().as_str().to_string(),
                header.value.as_str().to_string(),
            );
        }

        // Get the body
        let mut body = String::new();
        origin.as_reader().read_to_string(&mut body).unwrap();

        // Get the querystring
        let url = origin.url();
        let params;
        if url.contains("?") {
            let query = url.rsplitn(2, "?").nth(0).unwrap();
            params = params_from_query(query);
        } else {
            params = HashMap::new();
        }

        Request {
            source: source,
            headers: headers,
            params: params,
            body: body,
        }
    }
}


pub fn params_from_query(query: &str) -> HashMap<String, String> {
    let mut hashmap = HashMap::new();
    for (a, b) in form_urlencoded::parse(query.as_bytes()).into_owned() {
        hashmap.insert(a, b);
    }
    hashmap
}
