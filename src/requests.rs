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

use common::prelude::*;
use web::WebRequest;
use providers::StatusEvent;


#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum RequestType {
    ExecuteHook,
    Ping,
    Invalid,
}


#[derive(Debug, Clone)]
pub enum Request {
    Web(WebRequest),
    Status(StatusEvent),
}

impl Request {
    pub fn web(&self) -> Result<&WebRequest> {
        if let Request::Web(ref req) = *self {
            Ok(req)
        } else {
            Err(ErrorKind::WrongRequestKind.into())
        }
    }

    pub fn status(&self) -> Result<&StatusEvent> {
        if let Request::Status(ref req) = *self {
            Ok(req)
        } else {
            Err(ErrorKind::WrongRequestKind.into())
        }
    }
}


impl From<WebRequest> for Request {
    fn from(from: WebRequest) -> Request {
        Request::Web(from)
    }
}


impl From<StatusEvent> for Request {
    fn from(from: StatusEvent) -> Request {
        Request::Status(from)
    }
}
