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

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::fmt;

use common::prelude::*;
use common::state::{State, IdKind, UniqueId};

use super::scheduled_job::ScheduledJob;
use super::types::ScriptId;


pub enum ProcessResult<S: ScriptsRepositoryTrait + 'static> {
    Rejected(ScheduledJob<S>),
    Executing,
}

impl<S: ScriptsRepositoryTrait + 'static> ProcessResult<S> {

    #[cfg(test)]
    pub fn executed(&self) -> bool {
        match *self {
            ProcessResult::Executing => true,
            ProcessResult::Rejected(..) => false,
        }
    }
}


#[derive(Clone)]
pub struct ThreadCompleter {
    thread: thread::Thread,
    busy: Arc<AtomicBool>,
    manual: bool,
}

impl ThreadCompleter {

    pub fn new(busy: Arc<AtomicBool>) -> Self {
        ThreadCompleter {
            thread: thread::current(),
            busy,
            manual: false,
        }
    }

    pub fn manual_mode(&mut self) {
        self.manual = true;
    }

    pub fn manual_complete(&self) {
        self.busy.store(false, Ordering::SeqCst);
        self.thread.unpark();
    }
}

impl Drop for ThreadCompleter {

    fn drop(&mut self) {
        if ! self.manual {
            self.manual_complete();
        }
    }
}


pub struct Thread<S: ScriptsRepositoryTrait + 'static> {
    id: UniqueId,
    handle: thread::JoinHandle<()>,

    last_running_id: Option<ScriptId<S>>,

    busy: Arc<AtomicBool>,
    should_stop: Arc<AtomicBool>,
    communication: Arc<Mutex<Option<ScheduledJob<S>>>>,
}

impl<S: ScriptsRepositoryTrait> Thread<S> {

    pub fn new<
        E: Fn(ScheduledJob<S>, ThreadCompleter) -> Result<()> + Send + 'static,
    >(executor: E, state: &Arc<State>) -> Self {
        let thread_id = state.next_id(IdKind::ThreadId);
        let busy = Arc::new(AtomicBool::new(false));
        let should_stop = Arc::new(AtomicBool::new(false));
        let communication = Arc::new(Mutex::new(None));

        let c_busy = busy.clone();
        let c_should_stop = should_stop.clone();
        let c_communication = communication.clone();

        let handle = thread::spawn(move || {
            let completer = ThreadCompleter::new(c_busy.clone());
            let result = Thread::inner_thread(
                c_busy, c_should_stop, c_communication, executor, completer,
            );

            if let Err(error) = result {
                error.pretty_print();
            }
        });

        Thread {
            id: thread_id,
            handle,

            last_running_id: None,

            busy,
            should_stop,
            communication,
        }
    }

    fn inner_thread<
        E: Fn(ScheduledJob<S>, ThreadCompleter) -> Result<()> + Send + 'static,
    >(
        busy: Arc<AtomicBool>,
        should_stop: Arc<AtomicBool>,
        comm: Arc<Mutex<Option<ScheduledJob<S>>>>,
        executor: E,
        completer: ThreadCompleter,
    ) -> Result<()>{

        loop {
            // Ensure the thread is stopped
            if should_stop.load(Ordering::SeqCst) {
                break;
            }

            if let Some(job) = comm.lock()?.take() {
                executor(job, completer.clone())?;

                // Wait for the job to be marked completed
                if busy.load(Ordering::SeqCst) {
                    thread::park();
                }

                // Don't park the thread, look for another job right away
                continue;
            }

            // Block the thread until a new job is available
            // This avoids wasting unnecessary resources
            thread::park();
        }

        Ok(())
    }

    pub fn process(&mut self, job: ScheduledJob<S>) -> ProcessResult<S> {
        // Reject the job if the thread is going to be stopped
        if self.should_stop.load(Ordering::SeqCst) {
            return ProcessResult::Rejected(job);
        }

        if self.busy() {
            return ProcessResult::Rejected(job);
        }

        if let Ok(mut mutex) = self.communication.lock() {
            // Update the current state
            self.busy.store(true, Ordering::SeqCst);
            self.last_running_id = Some(job.hook_id());

            // Tell the thread what job it should process
            *mutex = Some(job);

            // Wake the thread up
            self.handle.thread().unpark();

            return ProcessResult::Executing;
        }

        return ProcessResult::Rejected(job);
    }

    pub fn stop(self) {
        // Tell the thread to stop and wake it up
        self.should_stop.store(true, Ordering::SeqCst);
        self.handle.thread().unpark();

        // Wait for the thread to quit
        let _ = self.handle.join();
    }

    pub fn id(&self) -> UniqueId {
        self.id
    }

    pub fn currently_running(&self) -> Option<ScriptId<S>> {
        if self.busy.load(Ordering::SeqCst) {
            self.last_running_id
        } else {
            None
        }
    }

    pub fn busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }
}

impl<S: ScriptsRepositoryTrait> fmt::Debug for Thread<S> {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Thread {{ busy: {}, should_stop: {} }}",
            self.busy(),
            self.should_stop.load(Ordering::SeqCst),
        )
    }
}


#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;

    use common::state::State;
    use common::serial::Serial;
    use processor::scheduled_job::ScheduledJob;
    use processor::test_utils::*;

    use super::Thread;


    fn job(repo: &Repository<()>, name: &str) -> ScheduledJob<Repository<()>> {
        let job = repo.job(name, ()).expect("job does not exist");
        ScheduledJob::new(job, 0, Serial::zero())
    }


    fn timeout_until_true<F: Fn() -> bool>(func: F, error: &'static str) {
        let start = Instant::now();
        loop {
            if start.elapsed().as_secs() > 10 {
                panic!(error);
            }

            if func() {
                return;
            }
        }
    }


    #[test]
    fn test_thread_executes_a_job() {
        test_wrapper(|| {
            let executed = Arc::new(AtomicBool::new(false));
            let repo = Repository::new();
            let state = Arc::new(State::new());

            // Create a job that changes the "executed" bit
            let job_executed = executed.clone();
            repo.add_script("job", true, move |_| {
                job_executed.store(true, Ordering::SeqCst);
                Ok(())
            });

            // Start a new thread able to execute jobs
            let mut thread = Thread::new(|job, _| {
                job.execute(&())?;
                Ok(())
            }, &state);

            // Tell the thread to process that job
            assert!(thread.process(job(&repo, "job")).executed());

            // Wait until the thread processes the job
            timeout_until_true(|| {
                ! thread.busy()
            }, "The thread didn't process the job");

            // Ensure the thread is not busy
            assert!(executed.load(Ordering::SeqCst));

            thread.stop();

            Ok(())
        });
    }
}
