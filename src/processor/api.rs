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

use std::sync::{mpsc, Arc};

use common::prelude::*;
use common::state::State;
use common::structs::HealthDetails;

use processor::scheduler::{Scheduler, SchedulerInput};
#[cfg(test)]
use processor::scheduler::DebugDetails;
use processor::types::{Job, JobContext};


/// This struct allows you to spawn a new processor, stop it and get its
/// [`ProcessorApi`](struct.ProcessorApi.html).

#[derive(Debug)]
pub struct Processor<S: ScriptsRepositoryTrait + 'static> {
    input: mpsc::Sender<SchedulerInput<S>>,
    wait: mpsc::Receiver<()>,
}

impl<S: ScriptsRepositoryTrait> Processor<S> {
    /// Create a new processor with the provided configuration. The returned
    /// struct allows you to control it.
    pub fn new(
        max_threads: u16,
        hooks: Arc<S>,
        ctx: JobContext<S>,
        state: Arc<State>,
    ) -> Result<Self> {
        // Retrieve wanted information from the spawned thread
        let (input_send, input_recv) = mpsc::sync_channel(0);
        let (wait_send, wait_recv) = mpsc::channel();

        ::std::thread::spawn(move || {
            let inner = Scheduler::new(max_threads, hooks, ctx, state);
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

    /// Stop this processor, and return only when the processor is stopped.
    pub fn stop(self) -> Result<()> {
        // Ask the processor to stop
        self.input.send(SchedulerInput::StopSignal)?;
        self.wait.recv()?;

        Ok(())
    }

    /// Get a struct allowing you to control the processor.
    pub fn api(&self) -> ProcessorApi<S> {
        ProcessorApi {
            input: self.input.clone(),
        }
    }
}


/// This struct allows you to interact with a running processor.

#[derive(Debug, Clone)]
pub struct ProcessorApi<S: ScriptsRepositoryTrait> {
    input: mpsc::Sender<SchedulerInput<S>>,
}

impl<S: ScriptsRepositoryTrait> ProcessorApi<S> {
    #[cfg(test)]
    pub fn debug_details(&self) -> Result<DebugDetails<S>> {
        let (res_send, res_recv) = mpsc::channel();
        self.input.send(SchedulerInput::DebugDetails(res_send))?;
        Ok(res_recv.recv()?)
    }

    pub fn update_context(&self, ctx: JobContext<S>) -> Result<()> {
        self.input.send(SchedulerInput::UpdateContext(ctx))?;
        Ok(())
    }
}

impl<S: ScriptsRepositoryTrait> ProcessorApiTrait<S> for ProcessorApi<S> {
    fn queue(&self, job: Job<S>, priority: isize) -> Result<()> {
        self.input.send(SchedulerInput::Job(job, priority))?;
        Ok(())
    }

    fn health_details(&self) -> Result<HealthDetails> {
        let (res_send, res_recv) = mpsc::channel();
        self.input.send(SchedulerInput::HealthStatus(res_send))?;
        Ok(res_recv.recv()?)
    }

    fn cleanup(&self) -> Result<()> {
        self.input.send(SchedulerInput::Cleanup)?;
        Ok(())
    }

    fn lock(&self) -> Result<()> {
        self.input.send(SchedulerInput::Lock)?;
        Ok(())
    }

    fn unlock(&self) -> Result<()> {
        self.input.send(SchedulerInput::Unlock)?;
        Ok(())
    }
}
