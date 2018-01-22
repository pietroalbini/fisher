// Copyright (C) 2018 Pietro Albini
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

use std::path::{Path, PathBuf};
use std::fs;
use std::env;


/// Convert a path relative to the current directory, if possible.
///
/// This is used to display prettier error messages, and returns the original
/// path if it's not a subdirectory the current one.
fn relative_to_current<P: AsRef<Path>>(original_path: P) -> PathBuf {
    let mut result = PathBuf::new();

    let current = if let Ok(curr) = env::current_dir(){
        curr
    } else {
        return original_path.as_ref().to_path_buf();
    };
    let path = if let Ok(path) = fs::canonicalize(&original_path) {
        path
    } else {
        return original_path.as_ref().to_path_buf();
    };

    let mut current_iter = current.iter();
    for segment in path.iter() {
        if let Some(current_segment) = current_iter.next() {
            if segment != current_segment {
                return original_path.as_ref().to_path_buf();
            }
        } else {
            result.push(segment);
        }
    }

    result
}


error_chain! {
    foreign_links {
        Io(::std::io::Error);
        ParseInt(::std::num::ParseIntError);
        AddrParse(::std::net::AddrParseError);
        Json(::serde_json::Error);
        Nix(::nix::Error);
    }

    errors {
        // Hex parsing errors
        HexInvalidChar(chr: char) {
            description("invalid char in hex"),
            display("invalid char in hex: {}", chr),
        }
        HexInvalidLength {
            description("odd length for hex string"),
            display("odd length for hex string"),
        }

        // Time strings
        TimeStringInvalid(string: String) {
            description("invalid time string"),
            display("invalid time string: {}", string),
        }
        TimeStringInvalidChar(chr: char) {
            description("time string contains an invalid char"),
            display("the char '{}' isn't allowed in time strings", chr),
        }
        TimeStringExpectedNumber(pos: usize) {
            description("expected a number in the time string"),
            display("expected a number in position {}", pos),
        }

        // Requests errors
        NotBehindProxy {
            description("not behind enough proxies"),
            display("not behind enough proxies"),
        }
        WrongRequestKind {
            description("wrong request kind"),
            display("wrong request kind"),
        }

        // Rate limit config
        RateLimitConfigTooManySlashes {
            description("too many slashes present"),
            display("too many slashes present"),
        }

        // Providers errors
        ProviderNotFound(name: String) {
            description("provider not found"),
            display("unknown provider: {}", name),
        }
        ProviderGitHubInvalidEventName(name: String) {
            description("invalid GitHub event name"),
            display("invalid GitHub event name: {}", name),
        }
        ProviderGitLabInvalidEventName(name: String) {
            description("invalid GitLab event name"),
            display("invalid GitLab event name: {}", name),
        }

        // Broken things
        BrokenChannel {
            description("an internal communication channel is broken"),
            display("an internal communication channel is broken"),
        }
        PoisonedLock {
            description("an internal lock is poisoned"),
            display("an internal lock is poisoned"),
        }

        // Other errors
        BoxedError(boxed: Box<::std::error::Error + Send + Sync>) {
            description("generic error"),
            display("{}", boxed),
        }

        // Chained errors
        ScriptExecutionFailed(name: String) {
            description("script execution failed"),
            display("execution of the '{}' script failed", name),
        }
        ScriptParsingError(file: String, line: u32) {
            description("script parsing error"),
            display(
                "parsing of the script '{}' failed (at line {})",
                relative_to_current(file).to_string_lossy(), line,
            ),
        }
        RateLimitConfigError(string: String) {
            description("error while parsing the rate limit config"),
            display("error while parsing rate limit config '{}'", string),
        }
    }
}

impl Error {
    pub fn pretty_print(&self) {
        println!("Error: {}", self);
        for chain in self.iter().skip(1) {
            println!("  caused by: {}", chain);
        }
    }
}

impl<T> From<::std::sync::mpsc::SendError<T>> for Error {
    fn from(_: ::std::sync::mpsc::SendError<T>) -> Error {
        ErrorKind::BrokenChannel.into()
    }
}

impl From<::std::sync::mpsc::RecvError> for Error {
    fn from(_: ::std::sync::mpsc::RecvError) -> Error {
        ErrorKind::BrokenChannel.into()
    }
}

impl<T> From<::std::sync::PoisonError<T>> for Error {
    fn from(_: ::std::sync::PoisonError<T>) -> Error {
        ErrorKind::PoisonedLock.into()
    }
}

impl From<Box<::std::error::Error + Send + Sync>> for Error {
    fn from(err: Box<::std::error::Error + Send + Sync>) -> Error {
        ErrorKind::BoxedError(err).into()
    }
}
