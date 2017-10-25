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

use std::time::Duration;

use serde_json;

use common::prelude::*;
use common::structs::HealthDetails;


#[derive(Debug)]
pub enum Response {
    NotFound,
    Forbidden,
    BadRequest(Error),
    TooManyRequests(Duration),
    Unavailable,
    Ok,
    HealthStatus(HealthDetails),
}

impl Response {
    pub fn status(&self) -> u16 {
        match *self {
            Response::NotFound => 404,
            Response::Forbidden => 403,
            Response::BadRequest(..) => 400,
            Response::TooManyRequests(..) => 429,
            Response::Unavailable => 503,
            _ => 200,
        }
    }

    pub fn json(&self) -> String {
        serde_json::to_string(&match *self {
            Response::HealthStatus(ref details) => json!({
                "status": "ok",
                "result": details,
            }),
            Response::BadRequest(ref error) => json!({
                "status": "bad_request",
                "error_msg": format!("{}", error),
            }),
            Response::TooManyRequests(ref until) => json!({
                "status": "too_many_requests",
                "retry_after": until.as_secs(),
            }),
            _ => json!({
                "status": match *self {
                    Response::NotFound => "not_found",
                    Response::Forbidden => "forbidden",
                    Response::BadRequest(..) => "bad_request",
                    Response::TooManyRequests(..) => "too_many_requests",
                    Response::Unavailable => "unavailable",
                    Response::Ok | Response::HealthStatus(..) => "ok",
                },
            }),
        }).unwrap()
    }

    pub fn headers(&self) -> Option<Vec<String>> {
        match *self {
            Response::TooManyRequests(ref duration) => {
                Some(vec![
                    format!("Retry-After: {}", duration.as_secs()),
                ])
            },
            _ => None,
        }
    }
}


#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json;

    use common::prelude::*;
    use common::structs::HealthDetails;

    use super::Response;


    #[inline]
    fn j(input: String) -> serde_json::Value {
        serde_json::from_str(&input).unwrap()
    }


    #[test]
    fn test_not_found() {
        let response = Response::NotFound;
        assert_eq!(response.status(), 404);
        assert!(response.headers().is_none());

        // The result must be an object
        let json = j(response.json());
        let obj = json.as_object().unwrap();

        // The status must be "not_found"
        assert_eq!(
            obj.get("status").unwrap().as_str().unwrap(),
            "not_found"
        );
    }


    #[test]
    fn test_forbidden() {
        let response = Response::Forbidden;
        assert_eq!(response.status(), 403);
        assert!(response.headers().is_none());

        // The result must be an object
        let json = j(response.json());
        let obj = json.as_object().unwrap();

        // The status must be "forbidden"
        assert_eq!(
            obj.get("status").unwrap().as_str().unwrap(),
            "forbidden"
        );
    }


    #[test]
    fn test_bad_request() {
        // This is just a dummy error
        let error = Error::new(ErrorKind::NotBehindProxy);
        let error_msg = format!("{}", error);

        let response = Response::BadRequest(error);
        assert_eq!(response.status(), 400);
        assert!(response.headers().is_none());

        // The result must be an object
        let json = j(response.json());
        let obj = json.as_object().unwrap();

        // The status must be "forbidden"
        assert_eq!(
            obj.get("status").unwrap().as_str().unwrap(),
            "bad_request"
        );

        // The error_msg must be the error's message
        assert_eq!(
            obj.get("error_msg").unwrap().as_str().unwrap(),
            error_msg.as_str()
        );
    }

    #[test]
    fn test_too_many_requests() {
        let response = Response::TooManyRequests(Duration::from_secs(10));
        assert_eq!(response.status(), 429);

        // Ensure headers are correct
        assert_eq!(response.headers(), Some(vec![
            "Retry-After: 10".into(),
        ]));

        // Ensure the response is correct
        assert_eq!(j(response.json()), json!({
            "status": "too_many_requests",
            "retry_after": 10,
        }));
    }


    #[test]
    fn test_unavailable() {
        let response = Response::Unavailable;
        assert_eq!(response.status(), 503);
        assert!(response.headers().is_none());

        // The result must be an object
        let json = j(response.json());
        let obj = json.as_object().unwrap();

        // The status must be "unavailable"
        assert_eq!(
            obj.get("status").unwrap().as_str().unwrap(),
            "unavailable"
        );
    }


    #[test]
    fn test_ok() {
        let response = Response::Ok;
        assert_eq!(response.status(), 200);
        assert!(response.headers().is_none());

        // The result must be an object
        let json = j(response.json());
        let obj = json.as_object().unwrap();

        // The status must be "ok"
        assert_eq!(
            obj.get("status").unwrap().as_str().unwrap(),
            "ok"
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
        let json = j(response.json());
        let obj = json.as_object().unwrap();
        assert_eq!(response.status(), 200);
        assert!(response.headers().is_none());


        // The status must be "ok"
        assert_eq!(
            obj.get("status").unwrap().as_str().unwrap(),
            "ok"
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
