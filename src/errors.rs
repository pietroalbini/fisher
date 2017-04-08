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

use std::io;
use std::fmt;
use std::net;
use std::num;
use std::error::Error;
use std::sync::mpsc;

use serde_json;

pub type FisherResult<T> = Result<T, FisherError>;


/// This enum represents the error that occured.

#[derive(Debug)]
pub enum ErrorKind {
    /// The provider requested by an hook doesn't exist. The provider name is
    /// provided as the first parameter.
    ProviderNotFound(String),

    /// The input you provided was invalid. A more detailed error message is
    /// available in the first parameter.
    InvalidInput(String),

    #[doc(hidden)]
    NotBehindProxy,

    #[doc(hidden)]
    WrongRequestKind,

    #[doc(hidden)]
    InvalidHexChar(char),

    #[doc(hidden)]
    InvalidHexLength,

    #[doc(hidden)]
    BrokenChannel,

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

    #[doc(hidden)]
    GenericError(Box<Error + Send + Sync>),
}


#[derive(Debug)]
pub struct FisherError {
    kind: ErrorKind,

    // Additional information
    file: Option<String>,
    line: Option<u32>,
    hook: Option<String>,
}

impl FisherError {

    pub fn new(kind: ErrorKind) -> Self {
        FisherError {
            kind: kind,

            // Those can be filled after
            file: None,
            line: None,
            hook: None,
        }
    }

    pub fn set_file(&mut self, file: String) {
        self.file = Some(file);
    }

    pub fn set_line(&mut self, line: u32) {
        self.line = Some(line);
    }

    pub fn location(&self) -> Option<String> {
        if let Some(file) = self.file.clone() {
            if let Some(line) = self.line {
                Some(format!("file {}, line {}", file, line))
            } else {
                Some(format!("file {}", file))
            }
        } else {
            None
        }
    }

    pub fn set_hook(&mut self, hook: String) {
        self.hook = Some(hook);
    }

    pub fn processing(&self) -> Option<String> {
        self.hook.clone()
    }

    #[cfg(test)]
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}


impl Error for FisherError {

    fn description(&self) -> &str {
        match self.kind {
            ErrorKind::ProviderNotFound(..) =>
                "provider not found",
            ErrorKind::InvalidInput(..) =>
                "invalid input",
            ErrorKind::NotBehindProxy =>
                "not behind the proxies",
            ErrorKind::WrongRequestKind =>
                "wrong request kind",
            ErrorKind::InvalidHexChar(..) =>
                "invalid character in hex",
            ErrorKind::InvalidHexLength =>
                "invalid length of the hex",
            ErrorKind::BrokenChannel =>
                "internal communication channel crashed",
            ErrorKind::IoError(ref error) =>
                error.description(),
            ErrorKind::JsonError(ref error) =>
                error.description(),
            ErrorKind::AddrParseError(ref error) =>
                error.description(),
            ErrorKind::ParseIntError(..) =>
                "invalid number",
            ErrorKind::GenericError(ref error) =>
                error.description(),
        }
    }

    fn cause(&self) -> Option<&Error> {
        match self.kind {
            ErrorKind::IoError(ref error) => Some(error as &Error),
            ErrorKind::JsonError(ref error) => Some(error as &Error),
            _ => None,
        }
    }
}

impl fmt::Display for FisherError {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Get the correct description for the error
        let description = match self.kind {

            ErrorKind::ProviderNotFound(ref provider) =>
                format!("Provider {} not found", provider),

            ErrorKind::InvalidInput(ref error) =>
                format!("invalid input: {}", error),

            ErrorKind::NotBehindProxy =>
                "not behind the proxies".into(),

            ErrorKind::WrongRequestKind =>
                "wrong request kind".into(),

            ErrorKind::InvalidHexChar(chr) =>
                format!("{} is not valid hex", chr),

            ErrorKind::InvalidHexLength =>
                "invalid length of the hex".into(),

            ErrorKind::BrokenChannel =>
                "an internal communication channel crashed".into(),

            ErrorKind::IoError(ref error) =>
                format!("{}", error),

            ErrorKind::JsonError(ref error) =>
                format!("{}", error),

            ErrorKind::AddrParseError(ref error) =>
                format!("{}", error),

            ErrorKind::ParseIntError(..) =>
                "you didn't provide a valid number".into(),

            ErrorKind::GenericError(ref error) =>
                format!("{}", error),
        };

        write!(f, "{}", description)
    }
}


macro_rules! derive_error {
    ($from:path, $to:path) => {
        impl From<$from> for FisherError {

            fn from(error: $from) -> Self {
                FisherError::new($to(error))
            }
        }
    };
}


impl From<ErrorKind> for FisherError {

    fn from(error: ErrorKind) -> Self {
        FisherError::new(error)
    }
}


impl From<mpsc::RecvError> for FisherError {

    fn from(_: mpsc::RecvError) -> Self {
        FisherError::new(ErrorKind::BrokenChannel)
    }
}


impl<T> From<mpsc::SendError<T>> for FisherError {

    fn from(_: mpsc::SendError<T>) -> Self {
        FisherError::new(ErrorKind::BrokenChannel)
    }
}


derive_error!(io::Error, ErrorKind::IoError);
derive_error!(serde_json::Error, ErrorKind::JsonError);
derive_error!(net::AddrParseError, ErrorKind::AddrParseError);
derive_error!(num::ParseIntError, ErrorKind::ParseIntError);
derive_error!(Box<Error + Send + Sync>, ErrorKind::GenericError);


pub fn print_err<T>(result: Result<T, FisherError>) -> Result<T, FisherError> {
    // Show a nice error message
    if let Err(ref error) = result {
        println!("{} {}",
            ::ansi_term::Colour::Red.bold().paint("Error:"),
            error,
        );
        if let Some(location) = error.location() {
            println!("{} {}",
                ::ansi_term::Colour::Yellow.bold().paint("Location:"),
                location,
            );
        }
        if let Some(hook) = error.processing() {
            println!("{} {}",
                ::ansi_term::Colour::Yellow.bold().paint("While processing:"),
                hook,
            );
        }
    }

    result
}


pub fn unwrap<T>(result: Result<T, FisherError>) -> T {
    // Print the error message if necessary
    match print_err(result) {
        Err(..) => ::std::process::exit(1),
        Ok(t) => t,
    }
}
