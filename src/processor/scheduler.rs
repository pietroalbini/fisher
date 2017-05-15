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
use std::sync::{Arc, mpsc};

use fisher_common::prelude::*;
use fisher_common::state::{State, UniqueId};

use jobs::{Job, JobOutput, Context};
use hooks::Hooks;
use utils::Serial;

use super::thread::Thread;
use super::scheduled_job::ScheduledJob;


const STATUS_EVENTS_PRIORITY: isize = 1000;


#[cfg(test)]
#[derive(Debug)]
pub struct DebugDetails {
    pub waiting: HashMap<UniqueId, usize>,
}

#[cfg(test)]
impl DebugDetails {

    fn from_scheduler(scheduler: &Scheduler) -> Self {
        let waiting = scheduler.waiting.iter()
            .map(|(key, value)| (*key, value.len()))
            .collect();

        DebugDetails {
            waiting: waiting,
        }
    }
}


#[derive(Clone, Debug, Serialize)]
pub struct HealthDetails {
    pub queued_jobs: usize,
    pub busy_threads: u16,
    pub max_threads: u16,
}


#[derive(Clone)]
pub enum SchedulerInput {
    Job(Job, isize),
    HealthStatus(mpsc::Sender<HealthDetails>),
    ProcessOutput(JobOutput),

    Cleanup,

    #[cfg(test)] DebugDetails(mpsc::Sender<DebugDetails>),

    Lock,
    Unlock,

    StopSignal,
    JobEnded(UniqueId, UniqueId),
}


#[derive(Debug, Clone)]
pub struct SchedulerInternalApi {
    input: mpsc::Sender<SchedulerInput>,
}

impl SchedulerInternalApi {

    pub fn record_output(&self, output: JobOutput) -> Result<()> {
        self.input.send(SchedulerInput::ProcessOutput(output))?;
        Ok(())
    }

    pub fn job_ended(&self, thread: UniqueId, job: &ScheduledJob)
                     -> Result<()> {
        self.input.send(SchedulerInput::JobEnded(thread, job.hook_id()))?;
        Ok(())
    }
}


#[derive(Debug)]
pub struct Scheduler {
    max_threads: u16,
    hooks: Arc<Hooks>,
    jobs_context: Arc<Context>,
    state: Arc<State>,

    locked: bool,
    should_stop: bool,
    queue: BinaryHeap<ScheduledJob>,
    waiting: HashMap<UniqueId, BinaryHeap<ScheduledJob>>,
    threads: HashMap<UniqueId, Thread>,

    input_send: mpsc::Sender<SchedulerInput>,
    input_recv: mpsc::Receiver<SchedulerInput>,
}

impl Scheduler {

    pub fn new(max_threads: u16, hooks: Arc<Hooks>,
           environment: HashMap<String, String>, state: Arc<State>) -> Self {
        let (input_send, input_recv) = mpsc::channel();

        let jobs_context = Arc::new(Context {
            environment: environment,
        });

        // Populate the waiting HashMap with non-parallel hooks
        let mut waiting = HashMap::new();
        for hook in hooks.iter() {
            if ! hook.can_be_parallel() {
                waiting.insert(hook.id(), BinaryHeap::new());
            }
        }

        Scheduler {
            max_threads: max_threads,
            hooks: hooks,
            jobs_context: jobs_context,
            state: state,

            locked: false,
            should_stop: false,
            queue: BinaryHeap::new(),
            waiting: waiting,
            threads: HashMap::with_capacity(max_threads as usize),

            input_send: input_send,
            input_recv: input_recv,
        }
    }

    pub fn input(&self) -> mpsc::Sender<SchedulerInput> {
        self.input_send.clone()
    }

    pub fn run(mut self) -> Result<()> {
        for _ in 0..self.max_threads {
            self.spawn_thread();
        }

        let mut serial = Serial::zero();
        let mut to_schedule = Vec::new();
        while let Ok(input) = self.input_recv.recv() {
            match input {

                SchedulerInput::Job(job, priority) => {
                    self.queue_job(ScheduledJob::new(
                        job, priority, serial.clone(),
                    ));
                    self.run_jobs();

                    serial.next();
                },

                SchedulerInput::HealthStatus(return_to) => {
                    // Count the busy threads
                    let busy_threads = self.threads.values()
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
                },

                SchedulerInput::ProcessOutput(output) => {
                    if let Some(jobs) = self.hooks.jobs_after_output(output) {
                        for job in jobs {
                            to_schedule.push(ScheduledJob::new(
                                job, STATUS_EVENTS_PRIORITY, serial.clone(),
                            ));
                            serial.next();
                        }
                    }

                    // This is a separated step due to mutable borrows
                    for job in to_schedule.drain(..) {
                        self.queue_job(job);
                    }

                    self.run_jobs();
                },

                SchedulerInput::Cleanup => {
                    self.cleanup_threads();
                    self.cleanup_hooks();
                },

                #[cfg(test)]
                SchedulerInput::DebugDetails(return_to) => {
                    let details = DebugDetails::from_scheduler(&self);
                    let _ = return_to.send(details);
                },

                SchedulerInput::Lock => {
                    self.locked = true;
                },

                SchedulerInput::Unlock => {
                    self.locked = false;
                    self.run_jobs();
                },

                SchedulerInput::JobEnded(thread_id, hook_id) => {
                    // Mark the thread as idle
                    if let Some(mut thread) = self.threads.get_mut(&thread_id) {
                        thread.mark_idle();
                    }

                    // Put the highest-priority waiting job for this hook
                    // back in the queue
                    let mut push_back = None;
                    if let Some(mut waiting) = self.waiting.get_mut(&hook_id) {
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
                },

                SchedulerInput::StopSignal => {
                    self.should_stop = true;
                    self.cleanup_threads();

                    if self.threads.is_empty() {
                        break;
                    }
                },
            }
        }

        Ok(())
    }

    #[inline]
    fn spawn_thread(&mut self) {
        let api = SchedulerInternalApi {
            input: self.input_send.clone(),
        };

        let thread = Thread::new(api, self.jobs_context.clone(), &self.state);
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
        let mut queued = HashSet::with_capacity(self.queue.len());;
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
            if ! waiting.is_empty() {
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
                    // The process() method returns Some(ScheduledJob) if
                    // *IT'S BUSY* working on another job
                    if let Some(j) = thread.process(job) {
                        job = j;
                    } else {
                        continue 'main;
                    }
                }
                self.queue_job(job);
            }
            break;
        }
    }

    fn queue_job(&mut self, job: ScheduledJob) {
        let hook_id = job.hook_id();

        // Put the job in waiting if it can't be parallel and
        // it's already running
        if self.is_running(hook_id) {
            if let Some(mut waiting) = self.waiting.get_mut(&hook_id) {
                waiting.push(job);
                return;
            }
        }

        self.queue.push(job);
    }

    fn get_job(&mut self) -> Option<ScheduledJob> {
        loop {
            if let Some(job) = self.queue.pop() {
                let hook_id = job.hook_id();

                // Put the job in waiting if it can't be parallel and
                // it's already running
                if self.is_running(hook_id) {
                    if let Some(mut waiting) = self.waiting.get_mut(&hook_id) {
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

    fn is_running(&self, hook: UniqueId) -> bool {
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
    use std::fs::File;
    use std::io::Read;
    use std::collections::{HashMap, VecDeque};
    use std::sync::Arc;

    use fisher_common::state::State;

    use utils::testing::*;
    use requests::Request;

    use super::super::Processor;


    #[test]
    fn test_processor_starting() {
        let env = TestingEnv::new();

        let processor = Processor::new(
            1, env.hooks(), HashMap::new(), Arc::new(State::new()),
        ).unwrap();
        processor.stop().unwrap();

        env.cleanup();
    }


    #[test]
    fn test_processor_clean_stop() {
        let mut env = TestingEnv::new();

        let processor = Processor::new(
            1, env.hooks(), HashMap::new(), Arc::new(State::new()),
        ).unwrap();

        // Prepare a request
        let mut out = env.tempdir();
        out.push("ok");

        let mut req = dummy_web_request();
        req.params.insert("env".into(), out.to_str().unwrap().to_string());

        // Queue a dummy job
        let job = env.create_job("long.sh", Request::Web(req));
        processor.api().queue(job, 0).unwrap();

        // Exit immediately -- this forces the processor to wait since the job
        // sleeps for half a second
        processor.stop().unwrap();

        // Check if the job was not killed
        assert!(out.exists(), "job was killed before it completed");

        env.cleanup();
    }


    fn run_multiple_append(threads: u16, prioritized: bool) -> String {
        let mut env = TestingEnv::new();

        let processor = Processor::new(
            threads, env.hooks(), HashMap::new(), Arc::new(State::new()),
        ).unwrap();

        let api = processor.api();
        let mut out = env.tempdir();
        out.push("out");

        // Prevent jobs from being run
        api.lock().unwrap();

        // Queue ten different jobs
        let mut req;
        let mut job;
        let mut priority = 0;
        for chr in 0..10 {
            req = dummy_web_request();
            req.params.insert("env".into(), format!("{}>{}",
                out.to_str().unwrap(), chr,
            ));

            if prioritized {
                priority = chr / 2;
            }

            job = env.create_job("append-val.sh", Request::Web(req));
            api.queue(job, priority).unwrap();
        }

        // Allow the processor to work
        api.unlock().unwrap();

        processor.stop().unwrap();

        // Read the content of the file
        let mut file = File::open(&out).unwrap();
        let mut output = String::new();
        file.read_to_string(&mut output).unwrap();

        env.cleanup();

        output.replace("\n", "")
    }


    #[test]
    fn test_processor_one_thread_correct_order() {
        let output = run_multiple_append(1, false);
        assert_eq!(output.as_str(), "0123456789");
    }


    #[test]
    fn test_processor_one_thread_correct_order_prioritized() {
        let output = run_multiple_append(1, true);
        assert_eq!(output.as_str(), "8967452301");
    }


    #[test]
    fn test_processor_multiple_threads() {
        let output = run_multiple_append(4, false);
        assert_eq!(output.len(), 10);
    }

    #[test]
    fn test_non_parallel_processing() {
        let mut env = TestingEnv::new();

        let processor = Processor::new(
            2, env.hooks(), HashMap::new(), Arc::new(State::new()),
        ).unwrap();
        let api = processor.api();

        // Queue ten jobs
        let mut waiters = VecDeque::new();
        for _ in 0..10 {
            let mut waiting = env.waiting_job(false);
            api.queue(waiting.take_job().unwrap(), 0).unwrap();
            waiters.push_back(waiting);
        }

        // Get the status
        while let Some(mut waiting) = waiters.pop_front() {
            // Only one job should be running
            let mut status;
            loop {
                status = api.health_status().unwrap();
                if status.queued_jobs == waiters.len() {
                    break;
                } else if status.queued_jobs != waiters.len() + 1 {
                    panic!(
                        "Wrong number of queued jobs: {}", status.queued_jobs
                    );
                }
            }

            assert_eq!(status.busy_threads, 1);
            assert_eq!(status.max_threads, 2);

            waiting.unlock().unwrap();

            // Wait for the job to be executed
            while ! waiting.executed() {}
        }

        processor.stop().unwrap();

        env.cleanup();
    }

    #[test]
    fn test_health_status() {
        let mut env = TestingEnv::new();

        let processor = Processor::new(
            1, env.hooks(), HashMap::new(), Arc::new(State::new()),
        ).unwrap();
        let api = processor.api();

        // Queue a wait job
        let mut waiting = env.waiting_job(true);
        api.queue(waiting.take_job().unwrap(), 0).unwrap();

        // Queue ten extra jobs
        let mut req;
        let mut job;
        for _ in 0..10 {
            req = Request::Web(dummy_web_request());
            job = env.create_job("example.sh", req);
            api.queue(job, 0).unwrap();
        }

        // Get the health status of the processor
        let status = api.health_status().unwrap();

        // Check if the health details are correct
        assert_eq!(status.queued_jobs, 10);
        assert_eq!(status.busy_threads, 1);
        assert_eq!(status.max_threads, 1);

        // Create the file the first job is waiting for
        waiting.unlock().unwrap();

        processor.stop().unwrap();

        // The file should not exist -- the first job removes it
        assert!(waiting.executed());

        env.cleanup();
    }


    #[test]
    fn test_cleanup_hooks() {
        wrapper(|env| {
            let processor = Processor::new(
                1, env.hooks(), HashMap::new(), env.state(),
            )?;
            let api = processor.api();

            let mut waitings = VecDeque::new();
            for _ in 0..10 {
                let mut waiting = env.waiting_job(true);
                api.queue(waiting.take_job().unwrap(), 1)?;
                waitings.push_back(waiting);
            }

            let old_hook_id = env.hook_id_of("wait.sh").unwrap();

            let debug = api.debug_details()?;
            assert_eq!(debug.waiting.get(&old_hook_id), Some(&9));

            // Execute only 5 out of 10 waiting jobs
            for mut waiting in waitings.drain(..5) {
                waiting.unlock()?;
            }

            // Wait until the previous operation ended
            while api.health_status()?.queued_jobs != 4 { }

            let debug = api.debug_details()?;
            assert_eq!(debug.waiting.get(&old_hook_id), Some(&4));

            // Reload the hooks
            env.reload_hooks()?;

            let new_hook_id = env.hook_id_of("wait.sh").unwrap();
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

            for mut waiting in waitings.drain(..) {
                waiting.unlock()?;
            }

            // Wait until the previous operation ended
            while api.health_status()?.busy_threads != 0 { }

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
