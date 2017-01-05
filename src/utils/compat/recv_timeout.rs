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


#[cfg(fisher_backport_recv_timeout)]
mod backport {
    use std::time::{Duration, Instant};
    use std::sync::mpsc::{Receiver, TryRecvError};


    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub enum RecvTimeoutError {
        Timeout,
        Disconnected,
    }


    pub fn recv_timeout<T>(chan: &Receiver<T>, timeout: Duration)
                      -> Result<T, RecvTimeoutError> {
        let start = Instant::now();

        // This method is inefficient, unfortunately
        loop {
            match chan.try_recv() {
                Ok(data) => {
                    return Ok(data);
                },
                Err(err) => match err {
                    TryRecvError::Empty => {},
                    TryRecvError::Disconnected => {
                        return Err(RecvTimeoutError::Disconnected);
                    },
                },
            }

            if start.elapsed() >= timeout {
                return Err(RecvTimeoutError::Timeout);
            }
        }
    }
}


#[cfg(not(fisher_backport_recv_timeout))]
mod stable {
    use std::sync::mpsc::Receiver;
    use std::time::Duration;

    pub use std::sync::mpsc::RecvTimeoutError;


    #[inline]
    pub fn recv_timeout<T>(recv: &Receiver<T>, timeout: Duration)
                      -> Result<T, RecvTimeoutError> {
        recv.recv_timeout(timeout)
    }
}


#[cfg(fisher_backport_recv_timeout)]
pub use self::backport::*;

#[cfg(not(fisher_backport_recv_timeout))]
pub use self::stable::*;
