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

use std::io;
use std::fmt;
use std::error::Error;
use std::convert::From;


pub type FisherResult<T> = Result<T, FisherError>;


pub enum FisherError {
    ProviderNotFound(String, String),

    // Generic IO Error
    IoError(io::Error),
}

impl Error for FisherError {

    fn description(&self) -> &str {
        match *self {
            FisherError::ProviderNotFound(..) =>
                "provider not found",
            FisherError::IoError(ref error) =>
                error.description(),
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            FisherError::IoError(ref error) => Some(error as &Error),
            _ => None,
        }
    }
}

impl fmt::Display for FisherError {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Get the correct description for the error
        let description = match *self {

            FisherError::ProviderNotFound(ref provider, ref file) =>
                format!("Provider {} not found (in file {})", provider, file),

            FisherError::IoError(ref error) =>
                format!("{}", error),
        };

        write!(f, "{}", description)
    }
}

impl fmt::Debug for FisherError {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<FisherError: {}>", self.description())
    }
}


impl From<io::Error> for FisherError {

    fn from(error: io::Error) -> Self {
        FisherError::IoError(error)
    }
}


pub fn abort<R, E: Error>(result: Result<R, E>) -> R {
    // Exit if the result is an error
    if result.is_err() {
        // Show a nice error message
        println!("{} {}",
            ::ansi_term::Colour::Red.bold().paint("Error:"),
            result.err().unwrap()
        );

        // And then exit
        ::std::process::exit(1);
    }

    result.unwrap()
}
