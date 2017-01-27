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

use std::collections::VecDeque;
use std::sync::{Arc, mpsc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::fmt;

use jobs::Job;
use hooks::Hooks;
use requests::Request;
use providers::StatusEvent;
use errors;


pub struct ProcessorManager {
    input: Option<mpsc::Sender<ProcessorInput>>,
    stop_wait: Option<mpsc::Receiver<()>>,
}

impl ProcessorManager {

    pub fn new() -> ProcessorManager {
        ProcessorManager {
            input: None,
            stop_wait: None,
        }
    }

    pub fn start(&mut self, max_threads: u16, hooks: Arc<Hooks>) {
        // This is used to retrieve the input we want from the child thread
        let (input_send, input_recv) = mpsc::sync_channel(0);

        // This is used by the thread to notify the processor it completed its
        // work, in order to block execution when stopping fisher
        let (stop_wait_send, stop_wait_recv) = mpsc::sync_channel(0);

        ::std::thread::spawn(move || {
            let (mut processor, input) = Processor::new(max_threads, hooks);

            // Send the input back to the parent thread
            input_send.send(input).unwrap();

            processor.run();

            // Notify ProcessorManager the thread did its work
            stop_wait_send.send(()).unwrap();
        });

        self.input = Some(input_recv.recv().unwrap());
        self.stop_wait = Some(stop_wait_recv);
    }

    pub fn stop(&self) {
        if let Some(ref input) = self.input {
            // Tell the processor to exit as soon as possible
            input.send(ProcessorInput::StopSignal).unwrap();

            // Wait until the processor did its work
            if let Some(ref stop_wait) = self.stop_wait {
                let _ = stop_wait.recv();
            }
        }
    }

    pub fn input(&self) -> Option<mpsc::Sender<ProcessorInput>> {
        self.input.clone()
    }
}


#[derive(Clone)]
pub enum ProcessorInput {
    StopSignal,
    Job(Job),
    HealthStatus(mpsc::Sender<HealthDetails>),
    JobEnded,
}


#[derive(Debug)]
struct Processor {
    jobs: VecDeque<Job>,
    hooks: Arc<Hooks>,

    should_stop: bool,
    threads: Vec<Thread>,
    max_threads: u16,

    input_recv: mpsc::Receiver<ProcessorInput>,
    input_send: mpsc::Sender<ProcessorInput>,
}

impl Processor {

    pub fn new(max_threads: u16, hooks: Arc<Hooks>)
               -> (Processor, mpsc::Sender<ProcessorInput>) {
        // Create the channel for the input
        let (input_send, input_recv) = mpsc::channel();

        let processor = Processor {
            jobs: VecDeque::new(),
            hooks: hooks,

            should_stop: false,
            threads: Vec::new(),
            max_threads: max_threads,

            input_recv: input_recv,
            input_send: input_send.clone(),
        };

        // Return both the processor and the input_send
        (processor, input_send)
    }

    pub fn run(&mut self) {
        // Create the needed threads
        for _ in 0..self.max_threads {
            self.spawn_thread();
        }

        while let Ok(input) = self.input_recv.recv() {
            match input {
                ProcessorInput::StopSignal => {
                    self.should_stop = true;
                    self.cleanup_threads();

                    // Exit if no more threads are left
                    if self.threads.is_empty() {
                        break;
                    }
                },
                ProcessorInput::Job(job) => {
                    self.run_jobs(job, false);
                },
                ProcessorInput::JobEnded => {
                    if let Some(job) = self.jobs.pop_front() {
                        self.run_jobs(job, true);
                    } else if self.should_stop {
                        // Clean up remaining threads
                        self.cleanup_threads();

                        // Exit if no more threads are left
                        if self.threads.is_empty() {
                            break;
                        }
                    }
                },
                ProcessorInput::HealthStatus(return_to) => {
                    return_to.send(HealthDetails {
                        queued_jobs: self.jobs.len(),
                        busy_threads: self.busy_threads(),
                        max_threads: self.max_threads,
                    }).unwrap();
                }
            }
        }
    }

    pub fn busy_threads(&self) -> u16 {
        let mut result = 0;

        for thread in &self.threads {
            if thread.busy() {
                result += 1;
            }
        }

        result
    }

    fn spawn_thread(&mut self) {
        self.threads.push(Thread::new(
            self.input_send.clone(), self.hooks.clone()
        ));
    }

    fn cleanup_threads(&mut self) {
        // This is done in two steps: the list of threads to remove is
        // computed, and then each marked thread is stopped
        let mut to_remove = Vec::with_capacity(self.threads.len());

        let mut remaining = self.threads.len();
        for (i, thread) in self.threads.iter().enumerate() {
            if thread.busy() {
                continue;
            }

            if self.should_stop || remaining > self.max_threads as usize {
                to_remove.push(i);
                remaining -= 1;
            }
        }

        for (removed, one) in to_remove.iter().enumerate() {
            let thread = self.threads.remove(one - removed);
            thread.stop();
        }
    }

    fn run_jobs(&mut self, mut job: Job, push_front: bool) {
        // Here there is a loop so if for some reason there are multiple
        // threads available and there are enough elements in the queue,
        // all of them are processed
        loop {
            // If the job *failed*
            if let Some(job) = self.run_job(job) {
                if push_front {
                    self.jobs.push_front(job);
                } else {
                    self.jobs.push_back(job);
                }
                return;
            }

            if let Some(j) = self.jobs.pop_front() {
                job = j;
            } else {
                return;
            }
        }
    }

    // Here, None equals to success, and Some(job) equals to failure
    fn run_job(&mut self, mut job: Job) -> Option<Job> {
        // Try to process the job in each thread
        for thread in &self.threads {
            // If Some(Job) is returned, the thread was busy
            if let Some(j) = thread.process(job) {
                // Continue with the loop, moving ownership of the job
                job = j;
            } else {
                return None;
            }
        }

        Some(job)
    }
}


#[derive(Debug)]
enum ThreadInput {
    Process(Job),
    StopSignal,
}


struct Thread {
    should_stop: bool,
    busy: Arc<AtomicBool>,

    handle: thread::JoinHandle<()>,
    input: mpsc::Sender<ThreadInput>,
}

impl Thread {

    pub fn new(processor_input: mpsc::Sender<ProcessorInput>,
               hooks: Arc<Hooks>) -> Thread {
        let (input_send, input_recv) = mpsc::channel();
        let busy = Arc::new(AtomicBool::new(false));

        let busy_inner = busy.clone();
        let handle = thread::spawn(move || {
            for input in input_recv.iter() {
                match input {
                    // A new job should be processed
                    ThreadInput::Process(job) => {
                        let result = job.process();

                        // Display the error if there is one
                        match result {
                            Ok(output) => {
                                let event = if output.success {
                                    StatusEvent::JobCompleted(output)
                                } else {
                                    StatusEvent::JobFailed(output)
                                };
                                let kind = event.kind();

                                let mut status_job;
                                let mut status_result;
                                for hp in hooks.status_hooks_iter(kind) {
                                    status_job = Job::new(
                                        hp.hook.clone(),
                                        Some(hp.provider.clone()),
                                        Request::Status(event.clone()),
                                    );
                                    status_result = status_job.process();

                                    if let Err(mut error) = status_result {
                                        error.set_hook(hp.hook.name().into());
                                        let _ = errors::print_err::<()>(Err(error));
                                    }
                                }
                            },
                            Err(mut error) => {
                                error.set_hook(job.hook_name().into());
                                let _ = errors::print_err::<()>(Err(error));
                            }
                        }

                        busy_inner.store(false, Ordering::Relaxed);
                        processor_input.send(
                            ProcessorInput::JobEnded
                        ).unwrap();
                    },

                    // Please stop, thanks!
                    ThreadInput::StopSignal => break,
                }
            }
        });

        Thread {
            should_stop: false,
            busy: busy,
            handle: handle,
            input: input_send,
        }
    }

    // Here, None equals to success, and Some(job) equals to failure
    pub fn process(&self, job: Job) -> Option<Job> {
        // Do some consistency checks
        if self.should_stop || self.busy() {
            return Some(job);
        }

        self.busy.store(true, Ordering::Relaxed);
        self.input.send(ThreadInput::Process(job)).unwrap();

        None
    }

    pub fn stop(mut self) {
        self.should_stop = true;
        self.input.send(ThreadInput::StopSignal).unwrap();

        self.handle.join().unwrap();
    }

    #[inline]
    pub fn busy(&self) -> bool {
        self.busy.load(Ordering::Relaxed)
    }
}

impl fmt::Debug for Thread {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Thread {{ busy: {}, should_stop: {} }}",
            self.busy(), self.should_stop,
        )
    }
}


#[derive(Clone, Debug, Serialize)]
pub struct HealthDetails {
    pub queued_jobs: usize,
    pub busy_threads: u16,
    pub max_threads: u16,
}


#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Read;
    use std::sync::mpsc;

    use utils::testing::*;
    use requests::Request;

    use super::{ProcessorManager, ProcessorInput};


    #[test]
    fn test_processor_starting() {
        let env = TestingEnv::new();

        let mut processor = ProcessorManager::new();
        processor.start(1, env.hooks());
        processor.stop();

        env.cleanup();
    }


    #[test]
    fn test_processor_clean_stop() {
        let mut env = TestingEnv::new();

        let mut processor = ProcessorManager::new();
        processor.start(1, env.hooks());

        // Prepare a request
        let mut out = env.tempdir();
        out.push("ok");

        let mut req = dummy_web_request();
        req.params.insert("env".into(), out.to_str().unwrap().to_string());

        // Queue a dummy job
        let job = env.create_job("long", Request::Web(req));
        processor.input().unwrap().send(ProcessorInput::Job(job)).unwrap();

        // Exit immediately -- this forces the processor to wait since the job
        // sleeps for half a second
        processor.stop();

        // Check if the job was not killed
        assert!(out.exists(), "job was killed before it completed");

        env.cleanup();
    }


    fn run_multiple_append(threads: u16) -> String {
        let mut env = TestingEnv::new();

        let mut processor = ProcessorManager::new();
        processor.start(threads, env.hooks());

        let input = processor.input().unwrap();
        let mut out = env.tempdir();
        out.push("out");

        // Queue ten different jobs
        let mut req;
        let mut job;
        for chr in 0..10 {
            req = dummy_web_request();
            req.params.insert("env".into(), format!("{}>{}",
                out.to_str().unwrap(), chr,
            ));

            job = env.create_job("append-val", Request::Web(req));
            input.send(ProcessorInput::Job(job)).unwrap();
        }

        processor.stop();

        // Read the content of the file
        let mut file = File::open(&out).unwrap();
        let mut output = String::new();
        file.read_to_string(&mut output).unwrap();

        env.cleanup();

        output.replace("\n", "")
    }


    #[test]
    fn test_processor_one_thread_correct_order() {
        let output = run_multiple_append(1);
        assert_eq!(output.as_str(), "0123456789");
    }


    #[test]
    fn test_processor_multiple_threads() {
        let output = run_multiple_append(4);
        assert_eq!(output.len(), 10);
    }

    #[test]
    fn test_health_status() {
        let mut env = TestingEnv::new();

        let mut processor = ProcessorManager::new();
        processor.start(1, env.hooks());

        let input = processor.input().unwrap();
        let mut out = env.tempdir();
        out.push("ok");

        // Queue a wait job
        let mut req = dummy_web_request();
        req.params.insert("env".into(), out.to_str().unwrap().to_string());
        let job = env.create_job("wait", Request::Web(req));
        input.send(ProcessorInput::Job(job)).unwrap();

        // Queue ten extra jobs
        let mut req;
        let mut job;
        for _ in 0..10 {
            req = Request::Web(dummy_web_request());
            job = env.create_job("example", req);
            input.send(ProcessorInput::Job(job)).unwrap();
        }

        // Get the health status of the processor
        let (status_send, status_recv) = mpsc::channel();
        input.send(ProcessorInput::HealthStatus(status_send)).unwrap();
        let status = status_recv.recv().unwrap();

        // Check if the health details are correct
        assert_eq!(status.queued_jobs, 10);
        assert_eq!(status.busy_threads, 1);
        assert_eq!(status.max_threads, 1);

        // Create the file the first job is waiting for
        File::create(&out).unwrap();

        processor.stop();

        // The file should not exist -- the first job removes it
        assert!(! out.exists());

        env.cleanup();
    }
}
