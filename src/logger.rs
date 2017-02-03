// Copyright (C) 2017 Pietro Albini
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

//! This module contains the implementation of the Fisher logging system. The
//! logging system allows for dispatching of all kinds of log events, and
//! supports multiple, user-defined listeners.
//!
//! You should hook into this if you need to know what's happening inside of
//! Fisher.

use std::sync::mpsc;

use errors::FisherError;


/// This is the type of a single log handler.
pub type LogHandler = Box<Fn(&LogEvent) + 'static + Send + Sync>;


/// This traits allows converting something into a log handler. Thanks to this,
/// the logging system can accept both boxed and unboxed handlers.
pub trait IntoLogHandler {
    fn into_handler(self) -> LogHandler;
}

impl IntoLogHandler for LogHandler {

    #[inline]
    fn into_handler(self) -> LogHandler {
        self
    }
}

impl<F> IntoLogHandler for F where F: Fn(&LogEvent) + 'static + Send + Sync {

    #[inline]
    fn into_handler(self) -> LogHandler {
        Box::new(self)
    }
}


/// This enum represents a single log event emitted by Fisher.
#[derive(Debug)]
pub enum LogEvent {
    /// An error occured inside of Fisher. The value contains all the details
    /// about the error, such as where it happened and why.
    Error(FisherError),
}

impl LogEvent {

    /// This method allows you to get the description of this event,
    /// regardless of its type.
    pub fn description(&self) -> String {
        use self::LogEvent::*;

        match *self {
            Error(ref error) => format!("{}", error),
        }
    }
}


/// This struct is the front end of the Fisher logging system. With this you
/// can add new listeners, and log new events.
#[derive(Debug, Clone)]
pub struct Logger {
    input: mpsc::Sender<LogThread>,
}

impl Logger {

    /// Create a new Logger instance. This method will start a new thread in
    /// the background, which will manage all the incoming log events.
    pub fn new() -> Logger {
        let input = LogThread::start();
        Logger {
            input: input,
        }
    }

    /// Add a new listener for log events. This allows you to process incoming
    /// events and dispatch them to your favourite logging solution.
    ///
    /// ```rust
    /// use fisher::logger::{Logger, LogEvent};
    ///
    /// let logger = Logger::new();
    ///
    /// logger.listen(|event: &LogEvent| {
    ///     println!("{}", event.description());
    /// });
    /// ```
    pub fn listen<F: IntoLogHandler>(&self, func: F) {
        self.input.send(LogThread::AddListener(func.into_handler())).unwrap();
    }

    /// Log a new event, notifying all the listeners about it.
    ///
    /// ```rust
    /// use fisher::logger::{Logger, LogEvent};
    /// use fisher::ErrorKind;
    ///
    /// let logger = Logger::new();
    ///
    /// // Log a dummy event, in this case a "wrong request kind" error
    /// logger.log(LogEvent::Error(ErrorKind::WrongRequestKind.into()));
    /// ```
    pub fn log(&self, event: LogEvent) {
        self.input.send(LogThread::NewEvent(event)).unwrap();
    }
}


enum LogThread {
    AddListener(LogHandler),
    NewEvent(LogEvent),
}

impl LogThread {

    fn start() -> mpsc::Sender<LogThread> {
        let (input_send, input_recv) = mpsc::channel();

        ::std::thread::spawn(move || {
            let mut listeners = Vec::new();

            while let Ok(input) = input_recv.recv() {
                match input {
                    // Add a new listener to this thread
                    LogThread::AddListener(listener) => {
                        listeners.push(listener);
                    },

                    LogThread::NewEvent(event) => {
                        // Call all the listeners with this event
                        for listener in &listeners {
                            (*listener)(&event);
                        }
                    }
                }
            }
        });

        input_send
    }
}


#[cfg(test)]
mod tests {
    use std::time::Duration;
    use std::sync::{mpsc, Mutex};

    use errors::ErrorKind;
    use super::{Logger, LogEvent};


    #[test]
    fn test_logger() {
        let logger = Logger::new();

        let (listener1_send, listener1_recv) = mpsc::channel();
        let listener1_mutex = Mutex::new(listener1_send);
        logger.listen(move |_: &LogEvent| {
            let _ = listener1_mutex.lock().unwrap().send(());
        });

        // Which event is sent is not important
        logger.log(LogEvent::Error(ErrorKind::WrongRequestKind.into()));

        listener1_recv
            .recv_timeout(Duration::from_secs(2))
            .expect("The listener didn't receive the event");

        let (listener2_send, listener2_recv) = mpsc::channel();
        let listener2_mutex = Mutex::new(listener2_send);
        logger.listen(move |_: &LogEvent| {
            let _ = listener2_mutex.lock().unwrap().send(());
        });

        // Which event is sent is not important
        logger.log(LogEvent::Error(ErrorKind::WrongRequestKind.into()));

        listener1_recv
            .recv_timeout(Duration::from_secs(2))
            .expect("The listener #1 didn't receive the event");
        listener2_recv
            .recv_timeout(Duration::from_secs(2))
            .expect("The listener #2 didn't receive the event");
    }
}
