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

use rustc_serialize::json;


pub type FisherResult<T> = Result<T, FisherError>;


pub enum ErrorKind {
    ProviderNotFound(String),
    HookExecutionFailed(Option<i32>, Option<i32>),

    // Derived errors
    IoError(io::Error),
    JsonError(json::DecoderError),
}


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

}


impl Error for FisherError {

    fn description(&self) -> &str {
        match self.kind {
            ErrorKind::ProviderNotFound(..) =>
                "provider not found",
            ErrorKind::HookExecutionFailed(..) =>
                "hook returned non-zero exit code",
            ErrorKind::IoError(ref error) =>
                error.description(),
            ErrorKind::JsonError(ref error) =>
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

            ErrorKind::HookExecutionFailed(exit_code_opt, signal_opt) =>
                if let Some(exit_code) = exit_code_opt {
                    // The hook returned an exit code
                    format!("hook returned non-zero exit code: {}", exit_code)
                } else if let Some(signal) = signal_opt {
                    // The hook was killed
                    format!("hook stopped with signal {}", signal)
                } else {
                    // This shouldn't happen...
                    "hook execution failed".to_string()
                },

            ErrorKind::IoError(ref error) =>
                format!("{}", error),

            // The default errors of rustc_serialize are really ugly btw
            ErrorKind::JsonError(ref error) => {
                use rustc_serialize::json::DecoderError;
                use rustc_serialize::json::ParserError;

                let message = match *error {

                    DecoderError::MissingFieldError(ref field) =>
                        format!("missing required field: {}", field),

                    DecoderError::ExpectedError(ref expected, ref found) =>
                        format!("expected {}, found {}", expected, found),

                    DecoderError::ParseError(ref pe) => match *pe {

                        ParserError::IoError(ref io_error) =>
                            format!("{}", io_error),

                        ParserError::SyntaxError(ref code, ref r, ref c) => {
                            let msg = json::error_str(code.clone());
                            format!("{} (line {}, column {})", msg, r, c)
                        },

                    },

                    _ => format!("{}", error),
                };

                format!("JSON error: {}", message)
            },
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
        FisherError::new(ErrorKind::IoError(error))
    }
}


impl From<json::DecoderError> for FisherError {

    fn from(error: json::DecoderError) -> Self {
        FisherError::new(ErrorKind::JsonError(error))
    }
}


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
        Err(..) => {
            ::std::process::exit(1);
        },
        Ok(t) => {
            return t;
        }
    }
}
