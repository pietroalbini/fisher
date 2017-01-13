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

use std::collections::{BTreeMap};

use rustc_serialize::json::{Json, ToJson};

use errors::FisherError;
use processor::HealthDetails;


#[derive(Debug)]
pub enum Response {
    NotFound,
    Forbidden,
    BadRequest(FisherError),
    Ok,
    HealthStatus(HealthDetails),
}

impl Response {

    pub fn status(&self) -> u16 {
        match *self {
            Response::NotFound => 404,
            Response::Forbidden => 403,
            Response::BadRequest(..) => 400,
            _ => 200,
        }
    }
}

impl ToJson for Response {

    fn to_json(&self) -> Json {
        let mut map = BTreeMap::new();

        map.insert("status".to_string(), match *self {
            Response::NotFound => "not_found",
            Response::Forbidden => "forbidden",
            Response::BadRequest(..) => "bad_request",
            Response::Ok => "ok",
            Response::HealthStatus(..) => "ok"
        }.to_string().to_json());

        if let Response::HealthStatus(ref details) = *self {
            map.insert("result".to_string(), details.to_json());
        }

        if let Response::BadRequest(ref error) = *self {
            map.insert("error_msg".into(), format!("{}", error).to_json());
        }

        Json::Object(map)
    }
}


#[cfg(test)]
mod tests {
    use rustc_serialize::json::ToJson;

    use processor::HealthDetails;
    use errors::{FisherError, ErrorKind};
    use super::Response;


    #[test]
    fn test_not_found() {
        let response = Response::NotFound;
        assert_eq!(response.status(), 404);

        // The result must be an object
        let json = response.to_json();
        let obj = json.as_object().unwrap();

        // The status must be "not_found"
        assert_eq!(
            obj.get("status").unwrap().as_string().unwrap(),
            "not_found".to_string()
        );
    }


    #[test]
    fn test_forbidden() {
        let response = Response::Forbidden;
        assert_eq!(response.status(), 403);

        // The result must be an object
        let json = response.to_json();
        let obj = json.as_object().unwrap();

        // The status must be "forbidden"
        assert_eq!(
            obj.get("status").unwrap().as_string().unwrap(),
            "forbidden".to_string()
        );
    }


    #[test]
    fn test_bad_request() {
        // This is just a dummy error
        let error = FisherError::new(ErrorKind::NotBehindProxy);
        let error_msg = format!("{}", error);

        let response = Response::BadRequest(error);
        assert_eq!(response.status(), 400);

        // The result must be an object
        let json = response.to_json();
        let obj = json.as_object().unwrap();

        // The status must be "forbidden"
        assert_eq!(
            obj.get("status").unwrap().as_string().unwrap(),
            "bad_request".to_string()
        );

        // The error_msg must be the error's message
        assert_eq!(
            obj.get("error_msg").unwrap().as_string().unwrap(),
            error_msg
        );
    }


    #[test]
    fn test_ok() {
        let response = Response::Ok;
        assert_eq!(response.status(), 200);

        // The result must be an object
        let json = response.to_json();
        let obj = json.as_object().unwrap();

        // The status must be "ok"
        assert_eq!(
            obj.get("status").unwrap().as_string().unwrap(),
            "ok".to_string()
        );
    }


    #[test]
    fn test_health_status() {
        let response = Response::HealthStatus(HealthDetails {
            queued_jobs: 1,
            busy_threads: 2,
            max_threads: 3,
        });

        // The result must be an object
        let json = response.to_json();
        let obj = json.as_object().unwrap();
        assert_eq!(response.status(), 200);


        // The status must be "ok"
        assert_eq!(
            obj.get("status").unwrap().as_string().unwrap(),
            "ok".to_string()
        );

        // It must have an object called "result"
        let result = obj.get("result").unwrap().as_object().unwrap();

        // The result must contain "queued_jobs", "busy_threads" and
        // "max_threads"
        assert_eq!(
            result.get("queued_jobs").unwrap().as_u64().unwrap(),
            1 as u64
        );
        assert_eq!(
            result.get("busy_threads").unwrap().as_u64().unwrap(),
            2 as u64
        );
        assert_eq!(
            result.get("max_threads").unwrap().as_u64().unwrap(),
            3 as u64
        )
    }
}
