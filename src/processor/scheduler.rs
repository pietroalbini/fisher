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

use std::collections::{BinaryHeap, HashMap, HashSet};
use std::time::Instant;
use std::sync::{mpsc, Arc, RwLock};

use common::prelude::*;
use common::state::{State, UniqueId};
use common::serial::Serial;
use common::structs::HealthDetails;

use super::thread::{ProcessResult, Thread, ThreadCompleter};
use super::scheduled_job::ScheduledJob;
use super::types::{Job, JobContext, JobOutput, ScriptId};


const STATUS_EVENTS_PRIORITY: isize = 1000;


#[cfg(test)]
#[derive(Debug)]
pub struct DebugDetails<S: ScriptsRepositoryTrait> {
    pub waiting: HashMap<ScriptId<S>, usize>,
}

#[cfg(test)]
impl<S: ScriptsRepositoryTrait> DebugDetails<S> {
    fn from_scheduler(scheduler: &Scheduler<S>) -> Self {
        let waiting = scheduler
            .waiting
            .iter()
            .map(|(key, value)| (*key, value.len()))
            .collect();

        DebugDetails { waiting: waiting }
    }
}


pub enum SchedulerInput<S: ScriptsRepositoryTrait> {
    Job(Job<S>, isize),
    HealthStatus(mpsc::Sender<HealthDetails>),
    ProcessOutput(JobOutput<S>),

    Cleanup,

    #[cfg(test)] DebugDetails(mpsc::Sender<DebugDetails<S>>),

    Lock,
    Unlock,

    UpdateContext(JobContext<S>),
    SetThreadsCount(u16),

    StopSignal,
    JobEnded(ScriptId<S>, ThreadCompleter),
}


#[derive(Debug)]
pub struct Scheduler<S: ScriptsRepositoryTrait + 'static> {
    max_threads: u16,
    hooks: Arc<S>,
    jobs_context: Arc<RwLock<Arc<JobContext<S>>>>,
    state: Arc<State>,

    locked: bool,
    should_stop: bool,
    queue: BinaryHeap<ScheduledJob<S>>,
    waiting: HashMap<ScriptId<S>, BinaryHeap<ScheduledJob<S>>>,
    threads: HashMap<UniqueId, Thread<S>>,

    input_send: mpsc::Sender<SchedulerInput<S>>,
    input_recv: mpsc::Receiver<SchedulerInput<S>>,

    last_cleanup: Instant,
}

impl<S: ScriptsRepositoryTrait> Scheduler<S> {
    pub fn new(
        max_threads: u16,
        hooks: Arc<S>,
        ctx: JobContext<S>,
        state: Arc<State>,
    ) -> Self {
        let (input_send, input_recv) = mpsc::channel();

        // Populate the waiting HashMap with non-parallel hooks
        let mut waiting = HashMap::new();
        for hook in hooks.iter() {
            if !hook.can_be_parallel() {
                waiting.insert(hook.id(), BinaryHeap::new());
            }
        }

        Scheduler {
            max_threads: max_threads,
            hooks: hooks,
            jobs_context: Arc::new(RwLock::new(Arc::new(ctx))),
            state: state,

            locked: false,
            should_stop: false,
            queue: BinaryHeap::new(),
            waiting: waiting,
            threads: HashMap::with_capacity(max_threads as usize),

            input_send: input_send,
            input_recv: input_recv,

            last_cleanup: Instant::now(),
        }
    }

    pub fn input(&self) -> mpsc::Sender<SchedulerInput<S>> {
        self.input_send.clone()
    }

    pub fn run(mut self) -> Result<()> {
        for _ in 0..self.max_threads {
            self.spawn_thread();
        }

        let mut serial = Serial::zero();
        let mut to_schedule = Vec::new();
        while let Ok(input) = self.input_recv.recv() {
            // Check if the periodic cleanup should be done now
            if self.last_cleanup.elapsed().as_secs() > 30 {
                self.cleanup_threads();
                self.cleanup_hooks();

                self.last_cleanup = Instant::now();
            }

            match input {
                SchedulerInput::Job(job, priority) => {
                    self.queue_job(
                        ScheduledJob::new(job, priority, serial.incr()),
                    );
                    self.run_jobs();
                }

                SchedulerInput::HealthStatus(return_to) => {
                    // Count the busy threads
                    let busy_threads = self.threads
                        .values()
                        .filter(|thread| thread.busy())
                        .count();

                    let mut queued_jobs = self.queue.len();
                    for waiting in self.waiting.values() {
                        queued_jobs += waiting.len();
                    }

                    return_to.send(HealthDetails {
                        queued_jobs: queued_jobs,
                        busy_threads: busy_threads as u16,
                        max_threads: self.max_threads,
                    })?;
                }

                SchedulerInput::ProcessOutput(output) => {
                    if let Some(jobs) = self.hooks.jobs_after_output(output) {
                        for job in jobs {
                            to_schedule.push(ScheduledJob::new(
                                job,
                                STATUS_EVENTS_PRIORITY,
                                serial.incr(),
                            ));
                        }
                    }

                    // This is a separated step due to mutable borrows
                    for job in to_schedule.drain(..) {
                        self.queue_job(job);
                    }

                    self.run_jobs();
                }

                SchedulerInput::Cleanup => {
                    self.cleanup_threads();
                    self.cleanup_hooks();
                }

                #[cfg(test)]
                SchedulerInput::DebugDetails(return_to) => {
                    let details = DebugDetails::from_scheduler(&self);
                    let _ = return_to.send(details);
                }

                SchedulerInput::Lock => {
                    self.locked = true;
                }

                SchedulerInput::Unlock => {
                    self.locked = false;
                    self.run_jobs();
                }

                SchedulerInput::UpdateContext(ctx) => {
                    let mut ptr = self.jobs_context.write().unwrap();
                    *ptr = Arc::new(ctx);
                }

                SchedulerInput::SetThreadsCount(max) => {
                    self.max_threads = max;

                    // Spawn new threads if the new maximum is higher, else
                    // start cleaning up old ones
                    if self.max_threads as usize > self.threads.len() {
                        for _ in self.threads.len()..self.max_threads as usize {
                            self.spawn_thread();
                        }
                    } else {
                        self.cleanup_threads();
                    }
                }

                SchedulerInput::JobEnded(hook_id, completer) => {
                    completer.manual_complete();

                    // Cleanup threads if there are more than enough
                    if self.threads.len() > self.max_threads as usize {
                        self.cleanup_threads();
                    }

                    // Put the highest-priority waiting job for this hook
                    // back in the queue
                    let mut push_back = None;
                    if let Some(waiting) = self.waiting.get_mut(&hook_id) {
                        push_back = waiting.pop();
                    }
                    if let Some(job) = push_back {
                        self.queue_job(job);
                    }

                    self.run_jobs();

                    if self.should_stop {
                        self.cleanup_threads();

                        if self.threads.is_empty() {
                            break;
                        }
                    }
                }

                SchedulerInput::StopSignal => {
                    self.should_stop = true;
                    self.cleanup_threads();

                    if self.threads.is_empty() {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    #[inline]
    fn spawn_thread(&mut self) {
        let ctx_lock = self.jobs_context.clone();
        let input = self.input_send.clone();

        let thread = Thread::new(
            move |job: ScheduledJob<S>, mut completer| {
                completer.manual_mode();

                let ctx = ctx_lock.read().unwrap().clone();
                let result = job.execute(&ctx);

                match result {
                    Ok(output) => {
                        input.send(SchedulerInput::ProcessOutput(output))?;
                    }
                    Err(error) => {
                        error.pretty_print();
                    }
                }

                input.send(SchedulerInput::JobEnded(job.hook_id(), completer))?;

                Ok(())
            },
            &self.state,
        );
        self.threads.insert(thread.id(), thread);
    }

    fn cleanup_threads(&mut self) {
        // This is done in two steps: the list of threads to remove is
        // computed, and then each marked thread is stopped
        let mut to_remove = Vec::with_capacity(self.threads.len());

        let mut remaining = self.threads.len();
        for (id, thread) in self.threads.iter() {
            if thread.busy() {
                continue;
            }

            if self.should_stop || remaining > self.max_threads as usize {
                to_remove.push(*id);
                remaining -= 1;
            }
        }

        for id in &to_remove {
            if let Some(thread) = self.threads.remove(id) {
                thread.stop();
            }
        }
    }

    fn cleanup_hooks(&mut self) {
        // Get a set of all the queued hooks
        let mut queued = HashSet::with_capacity(self.queue.len());
        for job in self.queue.iter() {
            queued.insert(job.hook_id());
        }

        // Remove old hooks from self.waiting
        let mut to_remove = Vec::with_capacity(self.waiting.len());
        for (hook_id, waiting) in self.waiting.iter() {
            // This hook wasn't deleted
            if self.hooks.id_exists(&hook_id) {
                continue;
            }

            // There are jobs waiting
            if !waiting.is_empty() {
                continue;
            }

            // There are jobs in the queue
            if queued.contains(&hook_id) {
                continue;
            }

            to_remove.push(*hook_id);
        }
        for hook_id in &to_remove {
            let _ = self.waiting.remove(&hook_id);
        }

        // Add new hooks
        for hook in self.hooks.iter() {
            if hook.can_be_parallel() {
                continue;
            }
            if self.waiting.contains_key(&hook.id()) {
                continue;
            }

            self.waiting.insert(hook.id(), BinaryHeap::new());
        }
    }

    fn run_jobs(&mut self) {
        if self.locked {
            return;
        }

        // Here there is a loop so if for some reason there are multiple
        // threads available and there are enough elements in the queue,
        // all of them are processed
        'main: loop {
            if let Some(mut job) = self.get_job() {
                // Try to run the job in a thread
                for thread in self.threads.values_mut() {
                    match thread.process(job) {
                        ProcessResult::Rejected(j) => job = j,
                        ProcessResult::Executing => continue 'main,
                    }
                }
                self.queue_job(job);
            }
            break;
        }
    }

    fn queue_job(&mut self, job: ScheduledJob<S>) {
        let hook_id = job.hook_id();

        // Put the job in waiting if it can't be parallel and
        // it's already running
        if self.is_running(hook_id) {
            if let Some(waiting) = self.waiting.get_mut(&hook_id) {
                waiting.push(job);
                return;
            }
        }

        self.queue.push(job);
    }

    fn get_job(&mut self) -> Option<ScheduledJob<S>> {
        loop {
            if let Some(job) = self.queue.pop() {
                let hook_id = job.hook_id();

                // Put the job in waiting if it can't be parallel and
                // it's already running
                if self.is_running(hook_id) {
                    if let Some(waiting) = self.waiting.get_mut(&hook_id) {
                        waiting.push(job);
                        continue;
                    }
                }

                return Some(job);
            } else {
                return None;
            }
        }
    }

    fn is_running(&self, hook: ScriptId<S>) -> bool {
        for thread in self.threads.values() {
            if thread.currently_running() == Some(hook) {
                return true;
            }
        }

        return false;
    }
}


#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{mpsc, Arc, Mutex};

    use common::prelude::*;
    use common::state::State;

    use super::super::test_utils::*;
    use super::super::Processor;


    #[test]
    fn test_processor_starting() {
        test_wrapper(|| {
            let repo = Arc::new(Repository::<()>::new());

            let processor =
                Processor::new(1, repo, (), Arc::new(State::new()))
                    .unwrap();
            processor.stop()?;

            Ok(())
        });
    }


    #[test]
    fn test_processor_clean_stop() {
        test_wrapper(|| {
            let repo = Repository::<()>::new();

            let (long_send, long_recv) = mpsc::channel();
            repo.add_script("long", true, move |_| {
                long_send.send(())?;
                Ok(())
            });

            let repo = Arc::new(repo);
            let processor = Processor::new(
                1,
                repo.clone(),
                (),
                Arc::new(State::new()),
            )?;

            processor.api().queue(repo.job("long", ()).unwrap(), 0)?;

            // Exit immediately -- this forces the processor to wait since the
            // job sleeps for half a second
            processor.stop()?;

            // Check if the job was not killed
            assert!(
                long_recv.try_recv().is_ok(),
                "job was killed before it completed"
            );

            Ok(())
        });
    }


    fn run_multiple_append(threads: u16, prioritized: bool) -> Result<String> {
        let repo = Repository::<char>::new();

        let (append_send, append_recv) = mpsc::channel();
        repo.add_script("append", true, move |arg| {
            append_send.send(arg)?;
            Ok(())
        });

        let repo = Arc::new(repo);
        let processor = Processor::new(
            threads,
            repo.clone(),
            (),
            Arc::new(State::new()),
        )?;

        let api = processor.api();

        // Prevent jobs from being run
        api.lock()?;

        // Queue ten different jobs
        let mut priority = 0;
        for chr in 0u8..10u8 {
            if prioritized {
                priority = chr / 2;
            }

            api.queue(
                repo.job("append", (chr + '0' as u8) as char).unwrap(),
                priority as isize,
            )?;
        }

        // Allow the processor to work
        api.unlock()?;

        processor.stop()?;

        // Collect the result from the channel
        let mut output = String::new();
        while let Ok(part) = append_recv.try_recv() {
            output.push(part);
        }
        Ok(output)
    }


    #[test]
    fn test_processor_one_thread_correct_order() {
        let output = run_multiple_append(1, false).unwrap();
        assert_eq!(output.as_str(), "0123456789");
    }


    #[test]
    fn test_processor_one_thread_correct_order_prioritized() {
        let output = run_multiple_append(1, true).unwrap();
        assert_eq!(output.as_str(), "8967452301");
    }


    #[test]
    fn test_processor_multiple_threads() {
        let output = run_multiple_append(4, false).unwrap();
        assert_eq!(output.len(), 10);
    }

    #[test]
    fn test_non_parallel_processing() {
        test_wrapper(|| {
            let repo = Repository::<Arc<Mutex<mpsc::Receiver<()>>>>::new();

            repo.add_script("wait", false, |recv| {
                recv.lock()?.recv()?;
                Ok(())
            });

            let repo = Arc::new(repo);
            let processor = Processor::new(
                2,
                repo.clone(),
                (),
                Arc::new(State::new()),
            )?;
            let api = processor.api();

            // Queue ten jobs
            let mut waiters = VecDeque::new();
            for _ in 0..10 {
                let (unlock_send, unlock_recv) = mpsc::channel();

                api.queue(
                    repo.job("wait", Arc::new(Mutex::new(unlock_recv)))
                        .unwrap(),
                    0,
                )?;
                waiters.push_back(unlock_send);
            }

            // Get the status
            while let Some(waiting) = waiters.pop_front() {
                // Only one job should be running
                let mut status;
                loop {
                    status = api.health_details()?;
                    if status.queued_jobs == waiters.len() {
                        break;
                    } else if status.queued_jobs != waiters.len() + 1 {
                        panic!(
                            "Wrong number of queued jobs: {}",
                            status.queued_jobs
                        );
                    }
                }

                assert_eq!(status.busy_threads, 1);
                assert_eq!(status.max_threads, 2);

                // Unlock this, thanks
                waiting.send(())?;
            }

            processor.stop()?;

            Ok(())
        });
    }

    #[test]
    fn test_health_details() {
        test_wrapper(|| {
            let repo =
                Repository::<Option<Arc<Mutex<mpsc::Receiver<()>>>>>::new();

            repo.add_script("noop", true, |_| Ok(()));
            repo.add_script("wait", true, |recv| {
                let recv = recv.unwrap();
                recv.lock()?.recv()?;
                Ok(())
            });

            let repo = Arc::new(repo);
            let processor = Processor::new(
                1,
                repo.clone(),
                (),
                Arc::new(State::new()),
            )?;
            let api = processor.api();

            // Queue a wait job
            let (waiting_send, waiting_recv) = mpsc::channel();
            api.queue(
                repo.job("wait", Some(Arc::new(Mutex::new(waiting_recv))))
                    .unwrap(),
                0,
            )?;

            // Queue ten extra jobs
            for _ in 0..10 {
                api.queue(repo.job("noop", None).unwrap(), 0)?;
            }

            // Get the health status of the processor
            let status = api.health_details()?;

            // Check if the health details are correct
            assert_eq!(status.queued_jobs, 10);
            assert_eq!(status.busy_threads, 1);
            assert_eq!(status.max_threads, 1);

            // Create the file the first job is waiting for
            waiting_send.send(())?;

            processor.stop()?;

            Ok(())
        });
    }


    #[test]
    fn test_cleanup_hooks() {
        test_wrapper(|| {
            let repo = Repository::<Arc<Mutex<mpsc::Receiver<()>>>>::new();

            repo.add_script("wait", false, |recv| {
                recv.lock()?.recv()?;
                Ok(())
            });

            let repo = Arc::new(repo);
            let processor = Processor::new(
                1,
                repo.clone(),
                (),
                Arc::new(State::new()),
            )?;
            let api = processor.api();

            let mut waitings = VecDeque::new();
            for _ in 0..10 {
                let (unlock_send, unlock_recv) = mpsc::channel();

                api.queue(
                    repo.job("wait", Arc::new(Mutex::new(unlock_recv)))
                        .unwrap(),
                    0,
                )?;
                waitings.push_back(unlock_send);
            }

            let old_hook_id = repo.script_id_of("wait").unwrap();

            let debug = api.debug_details()?;
            assert_eq!(debug.waiting.get(&old_hook_id), Some(&9));

            // Execute only 5 out of 10 waiting jobs
            for waiting in waitings.drain(..5) {
                waiting.send(())?;
            }

            // Wait until the previous operation ended
            while api.health_details()?.queued_jobs != 4 {}

            let debug = api.debug_details()?;
            assert_eq!(debug.waiting.get(&old_hook_id), Some(&4));

            // Reload the scripts
            repo.recreate_scripts();

            let new_hook_id = repo.script_id_of("wait").unwrap();
            assert!(new_hook_id != old_hook_id);

            // The new hook id shouldn't be present yet
            let debug = api.debug_details()?;
            assert_eq!(debug.waiting.get(&old_hook_id), Some(&4));
            assert_eq!(debug.waiting.get(&new_hook_id), None);

            // Execute a first cleanup
            api.cleanup()?;

            // Now the new hook id should be present, but with no hooks
            let debug = api.debug_details()?;
            assert_eq!(debug.waiting.get(&old_hook_id), Some(&4));
            assert_eq!(debug.waiting.get(&new_hook_id), Some(&0));

            for waiting in waitings.drain(..) {
                waiting.send(())?;
            }

            // Wait until the previous operation ended
            while api.health_details()?.busy_threads != 0 {}

            // Execute a second cleanup
            api.cleanup()?;

            // Now the old hook id should be gone
            let debug = api.debug_details()?;
            assert_eq!(debug.waiting.get(&old_hook_id), None);
            assert_eq!(debug.waiting.get(&new_hook_id), Some(&0));

            processor.stop()?;

            Ok(())
        });
    }
}
