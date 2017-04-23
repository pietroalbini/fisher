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

use std::thread::{JoinHandle, spawn};
use std::time::Duration;
use std::sync::mpsc;
use std::fmt;

use errors::{ErrorKind, FisherResult};


struct Task {
    handler: Box<Fn() + 'static + Send>,
    interval: u64,
    last_executed: Option<u64>,
}


enum TimerInput {
    AddTask(Task),
    Stop,

    #[cfg(test)] EnterTestMode,
    #[cfg(test)] TestTick(mpsc::Sender<()>),
}


pub struct Timer {
    input: mpsc::Sender<TimerInput>,
    handle: JoinHandle<()>,
}

impl Timer {

    pub fn new() -> Self {
        let (result_send, result_recv) = mpsc::channel();

        let handle = spawn(move || {
            let inner = TimerInner::new();
            result_send.send(inner.input()).unwrap();

            inner.run();
        });

        let input = result_recv.recv().unwrap();

        Timer {
            input: input,
            handle: handle,
        }
    }

    pub fn add_task<F: Fn() + 'static + Send>(&self, seconds: u64, handler: F)
                                              -> FisherResult<()> {
        self.input.send(TimerInput::AddTask(Task {
            handler: Box::new(handler),
            interval: seconds,
            last_executed: None,
        }))?;

        Ok(())
    }

    #[cfg(test)]
    pub fn enter_test_mode(&self) -> FisherResult<()> {
        self.input.send(TimerInput::EnterTestMode)?;
        Ok(())
    }

    #[cfg(test)]
    pub fn test_tick(&self) -> FisherResult<()> {
        let (result_send, result_recv) = mpsc::channel();
        self.input.send(TimerInput::TestTick(result_send))?;

        // Wait for the timer to do its things
        result_recv.recv()?;

        Ok(())
    }

    pub fn stop(self) -> FisherResult<()> {
        self.input.send(TimerInput::Stop)?;
        if self.handle.join().is_err() {
            return Err(ErrorKind::ThreadCrashed.into());
        }

        Ok(())
    }
}

impl fmt::Debug for Timer {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Timer")
    }
}


struct TimerInner {
    input_send: mpsc::Sender<TimerInput>,
    input_recv: mpsc::Receiver<TimerInput>,
    tasks: Vec<Task>,

    interval: Duration,
    elapsed: u64,
}

impl TimerInner {

    fn new() -> Self {
        let (input_send, input_recv) = mpsc::channel();

        TimerInner {
            input_send: input_send,
            input_recv: input_recv,
            tasks: Vec::new(),

            interval: Duration::new(1, 0),
            elapsed: 0,
        }
    }

    fn input(&self) -> mpsc::Sender<TimerInput> {
        self.input_send.clone()
    }

    fn run(mut self) {
        loop {
            match self.input_recv.recv_timeout(self.interval) {
                Ok(input) => match input {

                    TimerInput::AddTask(task) => {
                        self.tasks.push(task);
                    },

                    TimerInput::Stop => { break; },

                    #[cfg(test)]
                    TimerInput::EnterTestMode => {
                        self.interval = Duration::new(3600, 0);
                    },

                    #[cfg(test)]
                    TimerInput::TestTick(result) => {
                        self.run_tasks();
                        let _ = result.send(());
                    },

                },
                Err(reason) => match reason {
                    mpsc::RecvTimeoutError::Timeout => {
                        // One second elapsed
                        self.run_tasks();
                    },
                    mpsc::RecvTimeoutError::Disconnected => { break; },
                },
            }
        }
    }

    fn run_tasks(&mut self) {
        self.elapsed += 1;

        for task in self.tasks.iter_mut() {
            if let Some(last_executed) = task.last_executed {
                if last_executed + task.interval > self.elapsed {
                    continue;
                }
            }

            task.last_executed = Some(self.elapsed);
            (task.handler)()
        }
    }
}


#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::Timer;


    #[test]
    fn test_timer() {
        let timer = Timer::new();
        timer.enter_test_mode().unwrap();

        let (result_send, result_recv) = mpsc::channel();

        timer.add_task(2, move || {
            result_send.send(()).unwrap();
        }).unwrap();

        // The job is executed the first time after the first tick
        timer.test_tick().unwrap();
        assert!(result_recv.try_recv().is_ok());

        // Then after two ticks

        timer.test_tick().unwrap();
        timer.test_tick().unwrap();
        assert!(result_recv.try_recv().is_ok());

        timer.test_tick().unwrap();
        timer.test_tick().unwrap();
        assert!(result_recv.try_recv().is_ok());

        // Only one tick passed
        timer.test_tick().unwrap();
        assert!(result_recv.try_recv().is_err());

        timer.stop().unwrap();
    }
}
