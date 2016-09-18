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


pub enum JsonResponse {
    NotFound,
    Forbidden,
    BadRequest(FisherError),
    Ok,
    HealthStatus(HealthDetails),
}

impl ToJson for JsonResponse {

    fn to_json(&self) -> Json {
        let mut map = BTreeMap::new();

        map.insert("status".to_string(), match *self {
            JsonResponse::NotFound => "not_found",
            JsonResponse::Forbidden => "forbidden",
            JsonResponse::BadRequest(..) => "bad_request",
            JsonResponse::Ok => "ok",
            JsonResponse::HealthStatus(..) => "ok"
        }.to_string().to_json());

        if let JsonResponse::HealthStatus(ref details) = *self {
            map.insert("result".to_string(), details.to_json());
        }

        if let JsonResponse::BadRequest(ref error) = *self {
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
    use super::JsonResponse;


    #[test]
    fn test_not_found() {
        let response = JsonResponse::NotFound.to_json();

        // The result must be an object
        let obj = response.as_object().unwrap();

        // The status must be "not_found"
        assert_eq!(
            obj.get("status").unwrap().as_string().unwrap(),
            "not_found".to_string()
        );
    }


    #[test]
    fn test_forbidden() {
        let response = JsonResponse::Forbidden.to_json();

        // The result must be an object
        let obj = response.as_object().unwrap();

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

        let response = JsonResponse::BadRequest(error).to_json();

        // The result must be an object
        let obj = response.as_object().unwrap();

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
        let response = JsonResponse::Ok.to_json();

        // The result must be an object
        let obj = response.as_object().unwrap();

        // The status must be "ok"
        assert_eq!(
            obj.get("status").unwrap().as_string().unwrap(),
            "ok".to_string()
        );
    }


    #[test]
    fn test_health_status() {
        let response = JsonResponse::HealthStatus(HealthDetails {
            active_jobs: 1,
            queue_size: 2,
        }).to_json();

        // The result must be an object
        let obj = response.as_object().unwrap();

        // The status must be "ok"
        assert_eq!(
            obj.get("status").unwrap().as_string().unwrap(),
            "ok".to_string()
        );

        // It must have an object called "result"
        let result = obj.get("result").unwrap().as_object().unwrap();

        // The result must contain "active_jobs" and "queue_size"
        assert_eq!(
            result.get("active_jobs").unwrap().as_u64().unwrap(),
            1 as u64
        );
        assert_eq!(
            result.get("queue_size").unwrap().as_u64().unwrap(),
            2 as u64
        );
    }
}
