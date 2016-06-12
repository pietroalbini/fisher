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

use std::error::Error;
use std::fmt;


pub enum FisherError {
    ProviderNotFound(String, String),
    PathNotFound(String),
    PathNotADirectory(String),
}

impl FisherError {

    fn pretty_description(&self) -> String {
        match *self {
            FisherError::ProviderNotFound(ref hook, ref prov) =>
                format!("Provider {} not found (in hook {})", prov, hook),
            FisherError::PathNotFound(ref path) =>
                format!("Path {} doesn't exist", path),
            FisherError::PathNotADirectory(ref path) =>
                format!("Path {} isn't a directory", path),
        }
    }

}

impl Error for FisherError {

    fn description(&self) -> &str {
        match *self {
            FisherError::ProviderNotFound(..) =>
                "Provider not found",
            FisherError::PathNotFound(..) =>
                "Path doesn't exist",
            FisherError::PathNotADirectory(..) =>
                "Path isn't a directory",
        }
    }

    fn cause(&self) -> Option<&Error> {
        None
    }

}

impl fmt::Display for FisherError {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.pretty_description())
    }

}

impl fmt::Debug for FisherError {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<FisherError: {}>", self.pretty_description())
    }

}
