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

use std::collections::HashMap;
use std::sync::{Arc, mpsc};

use jobs::Job;
use hooks::Hooks;
use state::State;
use errors::FisherResult;

use super::scheduler::{Scheduler, SchedulerInput};

pub use super::scheduler::HealthDetails;


#[derive(Debug)]
pub struct Processor {
    input: mpsc::Sender<SchedulerInput>,
    wait: mpsc::Receiver<()>,
}

impl Processor {

    pub fn new(max_threads: u16, hooks: Arc<Hooks>,
               environment: HashMap<String, String>, state: Arc<State>)
               -> FisherResult<Self> {
        // Retrieve wanted information from the spawned thread
        let (input_send, input_recv) = mpsc::sync_channel(0);
        let (wait_send, wait_recv) = mpsc::channel();

        ::std::thread::spawn(move || {
            let inner = Scheduler::new(
                max_threads, hooks, environment, state,
            );
            input_send.send(inner.input()).unwrap();

            inner.run().unwrap();

            // Notify the main thread this exited
            wait_send.send(()).unwrap();
        });

        Ok(Processor {
            input: input_recv.recv()?,
            wait: wait_recv,
        })
    }

    pub fn stop(self) -> FisherResult<()> {
        // Ask the processor to stop
        self.input.send(SchedulerInput::StopSignal)?;
        self.wait.recv()?;

        Ok(())
    }

    pub fn api(&self) -> ProcessorApi {
        ProcessorApi {
            input: self.input.clone(),
        }
    }
}


#[derive(Debug, Clone)]
pub struct ProcessorApi {
    input: mpsc::Sender<SchedulerInput>,
}

impl ProcessorApi {

    #[cfg(test)]
    pub fn mock(input: mpsc::Sender<SchedulerInput>) -> Self {
        ProcessorApi {
            input: input,
        }
    }

    pub fn queue(&self, job: Job, priority: isize) -> FisherResult<()> {
        self.input.send(SchedulerInput::Job(job, priority))?;
        Ok(())
    }

    pub fn health_status(&self) -> FisherResult<HealthDetails> {
        let (res_send, res_recv) = mpsc::channel();
        self.input.send(SchedulerInput::HealthStatus(res_send))?;
        Ok(res_recv.recv()?)
    }

    #[cfg(test)]
    pub fn lock(&self) -> FisherResult<()> {
        self.input.send(SchedulerInput::Lock)?;
        Ok(())
    }

    #[cfg(test)]
    pub fn unlock(&self) -> FisherResult<()> {
        self.input.send(SchedulerInput::Unlock)?;
        Ok(())
    }
}
