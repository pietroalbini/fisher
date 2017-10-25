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

//! Error handling for Fisher.
//!
//! This module contains the definition of the [`Error`](struct.Error.html)
//! struct, which wraps all the details about any kind of error occuring in
//! Fisher. There is also the [`ErrorKind`](enum.ErrorKind.html) enum, which
//! contains exactly the kind of error occured.

use std::io;
use std::fmt;
use std::net;
use std::num;
use std::error::Error as StdError;
use std::sync::mpsc;
use std::sync;
use std::result::Result as StdResult;

use serde_json;
use ansi_term::Colour;


/// Convenience type alias to easily use Result with
/// [`Error`](struct.Error.html).

pub type Result<T> = StdResult<T, Error>;


/// This enum represents the kind of error that occured, with the details
/// about it.

#[derive(Debug)]
pub enum ErrorKind {
    /// The provider requested by an hook doesn't exist. The provider name is
    /// provided as the first parameter.
    ProviderNotFound(String),

    /// The input you provided was invalid. A more detailed error message is
    /// available in the first parameter.
    InvalidInput(String),

    /// The time string you provided was invalid. The provided time string is
    /// available in the first parameter.
    InvalidTimeString(String),

    /// The rate limits configuration you provided was invalid. The provided
    /// configuration string is available in the first parameter.
    InvalidRateLimitsConfig(String),

    /// The current request didn't travel across the configured number of
    /// proxies. This means the request was forged or the server is
    /// misconfigured.
    NotBehindProxy,

    /// The current request isn't of the required kind.
    WrongRequestKind,

    /// The character is not valid hex. The character is available in the
    /// first parameter.
    InvalidHexChar(char),

    /// The hex string has the wrong length.
    InvalidHexLength,

    /// An internal communication channel is broken.
    BrokenChannel,

    /// An internal lock is poisoned, probably due to a thread crash.
    PoisonedLock,

    /// An internal thread crashed.
    ThreadCrashed,

    /// An error occured while performing I/O operations. The underlying error
    /// is available as the first parameter.
    IoError(io::Error),

    /// An error occured while parsing some JSON. The underlying error is
    /// available as the first parameter.
    JsonError(serde_json::Error),

    /// An error occured while parsing an IP address. The underlying error is
    /// available as the first parameter.
    AddrParseError(net::AddrParseError),

    /// An error occured while parsing a number. The underlying error is
    /// available as the first parameter.
    ParseIntError(num::ParseIntError),

    /// A generic error, without a defined type
    GenericError(Box<StdError + Send + Sync>),

    #[doc(hidden)] Dummy,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                ErrorKind::ProviderNotFound(ref provider) => {
                    format!("Provider {} not found", provider)
                }

                ErrorKind::InvalidInput(ref error) => {
                    format!("invalid input: {}", error)
                }

                ErrorKind::InvalidTimeString(ref time_string) => {
                    format!("invalid time string: {}", time_string)
                }

                ErrorKind::InvalidRateLimitsConfig(ref config) => {
                    format!("invalid rate limits config: {}", config)
                }

                ErrorKind::NotBehindProxy => "not behind the proxies".into(),

                ErrorKind::WrongRequestKind => "wrong request kind".into(),

                ErrorKind::InvalidHexChar(chr) => {
                    format!("{} is not valid hex", chr)
                }

                ErrorKind::InvalidHexLength => {
                    "invalid length of the hex".into()
                }

                ErrorKind::BrokenChannel => {
                    "an internal communication channel crashed".into()
                }

                ErrorKind::PoisonedLock => {
                    "an internal lock was poisoned".into()
                }

                ErrorKind::ThreadCrashed => "an internal thread crashed".into(),

                ErrorKind::IoError(ref error) => format!("{}", error),

                ErrorKind::JsonError(ref error) => format!("{}", error),

                ErrorKind::AddrParseError(ref error) => format!("{}", error),

                ErrorKind::ParseIntError(..) => {
                    "you didn't provide a valid number".into()
                }

                ErrorKind::GenericError(ref error) => format!("{}", error),

                ErrorKind::Dummy => "dummy_error".into(),
            }
        )
    }
}




/// This enum represents where the error occured.

#[derive(Debug, PartialEq, Eq)]
pub enum ErrorLocation {
    /// The error occured in a file. The file name is available in the first
    /// parameter, while the line number (if present) is available in the
    /// second one.
    File(String, Option<u32>),

    /// The error occured while processing an hook. The hook name is available
    /// in the first parameter.
    HookProcessing(String),

    /// There is no information about where the error occured.
    Unknown,

    #[doc(hidden)] __NonExaustiveMatch,
}

impl fmt::Display for ErrorLocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ErrorLocation::File(ref path, line) => {
                write!(f, "file {}", path)?;
                if let Some(num) = line {
                    write!(f, ", on line {}", num)?;
                }

                Ok(())
            }

            ErrorLocation::HookProcessing(ref name) => {
                write!(f, "while processing {}", name)
            }

            ErrorLocation::Unknown => write!(f, ""),

            ErrorLocation::__NonExaustiveMatch => {
                panic!("You shouldn't use this.");
            }
        }
    }
}


/// This class represents an error that occured in Fisher.
///
/// It contains all the details known about it, and you can either access it
/// programmatically or display the error message to the user, already
/// formatted. It also support automatic conversion from the error types of
/// the libraries Fisher depends on.

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    location: ErrorLocation,
}

impl Error {
    /// Create a new error. You need to provide the kind of error that occured.
    ///
    /// ## Example
    ///
    /// ```rust
    /// # use fisher::common::errors::{Result, Error, ErrorKind};
    /// fn my_function() -> Result<()> {
    ///     let error = Error::new(ErrorKind::Dummy);
    ///     Err(error)
    /// }
    /// # fn main() {
    /// #   assert!(my_function().is_err());
    /// # }
    /// ```
    pub fn new(kind: ErrorKind) -> Self {
        Error {
            kind: kind,
            location: ErrorLocation::Unknown,
        }
    }

    /// Set the location where the error occured.
    pub fn set_location(&mut self, location: ErrorLocation) {
        self.location = location;
    }

    /// Get the location where the error occured. You can either access it
    /// programmatically or print a pretty version of it to the user.
    pub fn location(&self) -> &ErrorLocation {
        &self.location
    }

    /// Get the kind of error occured. You can either access it
    /// programmatically or print a pretty version of it to the user.
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    /// Show a nicely-formatted version of the error, usually for printing
    /// it to the user. The function uses ANSI formatting codes.
    ///
    /// ```rust
    /// # use fisher::common::errors::{Result, Error, ErrorKind};
    /// # fn do_work() -> Result<()> {
    /// #   Err(Error::new(ErrorKind::Dummy))
    /// # }
    /// if let Err(error) = do_work() {
    ///     error.pretty_print();
    /// }
    /// ```
    pub fn pretty_print(&self) {
        println!("{} {}", Colour::Red.bold().paint("Error:"), self);
        if self.location != ErrorLocation::Unknown {
            println!(
                "{} {}",
                Colour::Yellow.bold().paint("Location:"),
                self.location
            );
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match self.kind {
            ErrorKind::ProviderNotFound(..) => "provider not found",
            ErrorKind::InvalidInput(..) => "invalid input",
            ErrorKind::InvalidTimeString(..) => "invalid time string",
            ErrorKind::InvalidRateLimitsConfig(..) => {
                "invalid rate limits config"
            }
            ErrorKind::NotBehindProxy => "not behind the proxies",
            ErrorKind::WrongRequestKind => "wrong request kind",
            ErrorKind::InvalidHexChar(..) => "invalid character in hex",
            ErrorKind::InvalidHexLength => "invalid length of the hex",
            ErrorKind::BrokenChannel => {
                "internal communication channel crashed"
            }
            ErrorKind::PoisonedLock => "poisoned lock",
            ErrorKind::ThreadCrashed => "thread crashed",
            ErrorKind::IoError(ref error) => error.description(),
            ErrorKind::JsonError(ref error) => error.description(),
            ErrorKind::AddrParseError(ref error) => error.description(),
            ErrorKind::ParseIntError(..) => "invalid number",
            ErrorKind::GenericError(ref error) => error.description(),
            ErrorKind::Dummy => "dummy error",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match self.kind {
            ErrorKind::IoError(ref error) => Some(error as &StdError),
            ErrorKind::JsonError(ref error) => Some(error as &StdError),
            ErrorKind::AddrParseError(ref error) => Some(error as &StdError),
            ErrorKind::ParseIntError(ref error) => Some(error as &StdError),
            _ => None,
        }
    }
}

macro_rules! derive_error {
    ($from:path, $to:path) => {
        impl From<$from> for Error {

            fn from(error: $from) -> Self {
                Error::new($to(error))
            }
        }
    };
}

impl From<ErrorKind> for Error {
    fn from(error: ErrorKind) -> Self {
        Error::new(error)
    }
}

impl From<mpsc::RecvError> for Error {
    fn from(_: mpsc::RecvError) -> Self {
        Error::new(ErrorKind::BrokenChannel)
    }
}

impl<T> From<mpsc::SendError<T>> for Error {
    fn from(_: mpsc::SendError<T>) -> Self {
        Error::new(ErrorKind::BrokenChannel)
    }
}

impl<T> From<sync::PoisonError<T>> for Error {
    fn from(_: sync::PoisonError<T>) -> Self {
        Error::new(ErrorKind::PoisonedLock)
    }
}

derive_error!(io::Error, ErrorKind::IoError);
derive_error!(serde_json::Error, ErrorKind::JsonError);
derive_error!(net::AddrParseError, ErrorKind::AddrParseError);
derive_error!(num::ParseIntError, ErrorKind::ParseIntError);
derive_error!(Box<StdError + Send + Sync>, ErrorKind::GenericError);
