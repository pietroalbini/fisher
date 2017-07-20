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

//! Structs related to requests used by Fisher.

use std::collections::HashMap;
use std::net::IpAddr;

use prelude::*;
use structs::jobs::JobOutput;


/// The type of the incoming request.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum RequestType {
    /// Request that should execute an hook.
    ExecuteHook,

    /// Request received to notify everything works OK.
    Ping,

    /// Invalid request to reject.
    Invalid,
}


/// A web request coming from an HTTP client.
#[derive(Debug, Clone)]
pub struct WebRequest {
    /// The IP address of the source of the request.
    pub source: IpAddr,

    /// The headers of the request.
    pub headers: HashMap<String, String>,

    /// The params of the request.
    pub params: HashMap<String, String>,

    /// The body of the request.
    pub body: String,
}


/// An incoming request to Fisher.
#[derive(Debug, Clone)]
pub enum Request {
    /// A status event.
    Status(StatusEvent),

    /// A web request.
    Web(WebRequest),
}

impl Request {

    /// Get the [`WebRequest`](struct.WebRequest.html) or return an error.
    pub fn web(&self) -> Result<&WebRequest> {
        if let Request::Web(ref req) = *self {
            Ok(req)
        } else {
            Err(ErrorKind::WrongRequestKind.into())
        }
    }

    /// Get the [`StatusEvent`](enum.StatusEvent.html) or return an error.
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


/// An incoming status event.
#[derive(Debug, Clone)]
pub enum StatusEvent {
    /// Event triggered when a job successfully completes.
    JobCompleted(JobOutput),

    /// Event triggered when a job fails.
    JobFailed(JobOutput),
}

impl StatusEvent {

    /// The kind of status event.
    pub fn kind(&self) -> StatusEventKind {
        match *self {
            StatusEvent::JobCompleted(..) => StatusEventKind::JobCompleted,
            StatusEvent::JobFailed(..) => StatusEventKind::JobFailed,
        }
    }

    /// The name of the script that triggered the event.
    pub fn script_name(&self) -> &str {
        match *self {
            StatusEvent::JobCompleted(ref output) |
            StatusEvent::JobFailed(ref output) => &output.job.script_name,
        }
    }

    /// The IP that triggered the event.
    pub fn source_ip(&self) -> IpAddr {
        match *self {
            StatusEvent::JobCompleted(ref output) |
            StatusEvent::JobFailed(ref output) => output.job.ip,
        }
    }
}


/// The kind of [`StatusEvent`](enum.StatusEvent.html).
#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone, Deserialize)]
pub enum StatusEventKind {
    /// A JobCompleted event.
    #[serde(rename = "job_completed")]
    JobCompleted,

    /// A JobFailed event.
    #[serde(rename = "job_failed")]
    JobFailed,
}

impl StatusEventKind {

    /// Return the name of the event
    pub fn name(&self) -> &str {
        match *self {
            StatusEventKind::JobCompleted => "job_completed",
            StatusEventKind::JobFailed => "job_failed",
        }
    }
}
