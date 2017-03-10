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

use std::collections::{BinaryHeap, HashMap};
use std::sync::{Arc, mpsc};

use jobs::{Job, Context};
use hooks::Hooks;
use utils::Serial;
use errors::FisherResult;

use super::thread::Thread;
use super::scheduled_job::ScheduledJob;


#[derive(Clone, Debug, Serialize)]
pub struct HealthDetails {
    pub queued_jobs: usize,
    pub busy_threads: u16,
    pub max_threads: u16,
}


#[derive(Clone)]
pub enum ProcessorInput {
    Job(Job, isize),
    HealthStatus(mpsc::Sender<HealthDetails>),

    _Lock,
    _Unlock,

    _StopSignal,
    _JobEnded,
}


#[derive(Debug)]
pub struct Processor {
    input: mpsc::Sender<ProcessorInput>,
    wait: mpsc::Receiver<()>,
}

impl Processor {

    pub fn new(max_threads: u16, hooks: Arc<Hooks>,
               environment: HashMap<String, String>) -> FisherResult<Self> {
        // Retrieve wanted information from the spawned thread
        let (input_send, input_recv) = mpsc::sync_channel(0);
        let (wait_send, wait_recv) = mpsc::channel();

        ::std::thread::spawn(move || {
            let inner = InnerProcessor::new(
                max_threads, hooks, environment,
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
        self.input.send(ProcessorInput::_StopSignal)?;
        self.wait.recv()?;

        Ok(())
    }

    pub fn input(&self) -> mpsc::Sender<ProcessorInput> {
        self.input.clone()
    }
}


#[derive(Debug)]
struct InnerProcessor {
    max_threads: u16,
    hooks: Arc<Hooks>,
    jobs_context: Arc<Context>,

    locked: bool,
    should_stop: bool,
    queue: BinaryHeap<ScheduledJob>,
    threads: Vec<Thread>,

    input_send: mpsc::Sender<ProcessorInput>,
    input_recv: mpsc::Receiver<ProcessorInput>,
}

impl InnerProcessor {

    fn new(max_threads: u16, hooks: Arc<Hooks>,
           environment: HashMap<String, String>) -> Self {
        let (input_send, input_recv) = mpsc::channel();

        let jobs_context = Arc::new(Context {
            environment: environment,
        });

        InnerProcessor {
            max_threads: max_threads,
            hooks: hooks,
            jobs_context: jobs_context,

            locked: false,
            should_stop: false,
            queue: BinaryHeap::new(),
            threads: Vec::new(),

            input_send: input_send,
            input_recv: input_recv,
        }
    }

    fn input(&self) -> mpsc::Sender<ProcessorInput> {
        self.input_send.clone()
    }

    fn run(mut self) -> FisherResult<()> {
        for _ in 0..self.max_threads {
            self.spawn_thread();
        }

        let mut serial = Serial::zero();
        while let Ok(input) = self.input_recv.recv() {
            match input {

                ProcessorInput::Job(job, priority) => {
                    self.queue.push(ScheduledJob::new(
                        job, priority, serial.clone()
                    ));
                    self.run_jobs();

                    serial.next();
                },

                ProcessorInput::HealthStatus(return_to) => {
                    // Count the busy threads
                    let busy_threads = self.threads.iter()
                        .filter(|thread| thread.busy())
                        .count();

                    return_to.send(HealthDetails {
                        queued_jobs: self.queue.len(),
                        busy_threads: busy_threads as u16,
                        max_threads: self.max_threads,
                    })?;
                },

                ProcessorInput::_Lock => {
                    self.locked = true;
                },

                ProcessorInput::_Unlock => {
                    self.locked = false;
                    self.run_jobs();
                },

                ProcessorInput::_JobEnded => {
                    self.run_jobs();

                    if self.should_stop {
                        self.cleanup_threads();

                        if self.threads.is_empty() {
                            break;
                        }
                    }
                },

                ProcessorInput::_StopSignal => {
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
        self.threads.push(Thread::new(
            self.input_send.clone(), self.jobs_context.clone(),
            self.hooks.clone(),
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

    fn run_jobs(&mut self) {
        if self.locked {
            return;
        }

        // Here there is a loop so if for some reason there are multiple
        // threads available and there are enough elements in the queue,
        // all of them are processed
        'main: loop {
            if let Some(mut job) = self.queue.pop() {
                // Try to run the job in a thread
                for thread in &self.threads {
                    // The process() method returns Some(ScheduledJob) if
                    // *IT'S BUSY* working on another job
                    if let Some(j) = thread.process(job) {
                        job = j;
                    } else {
                        continue 'main;
                    }
                }
                self.queue.push(job);
            }
            break;
        }
    }
}


#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Read;
    use std::sync::mpsc;
    use std::collections::HashMap;

    use utils::testing::*;
    use requests::Request;

    use super::{Processor, ProcessorInput};


    #[test]
    fn test_processor_starting() {
        let env = TestingEnv::new();

        let processor = Processor::new(
            1, env.hooks(), HashMap::new()
        ).unwrap();
        processor.stop().unwrap();

        env.cleanup();
    }


    #[test]
    fn test_processor_clean_stop() {
        let mut env = TestingEnv::new();

        let processor = Processor::new(
            1, env.hooks(), HashMap::new()
        ).unwrap();

        // Prepare a request
        let mut out = env.tempdir();
        out.push("ok");

        let mut req = dummy_web_request();
        req.params.insert("env".into(), out.to_str().unwrap().to_string());

        // Queue a dummy job
        let job = env.create_job("long.sh", Request::Web(req));
        processor.input().send(ProcessorInput::Job(job, 0)).unwrap();

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
            threads, env.hooks(), HashMap::new()
        ).unwrap();

        let input = processor.input();
        let mut out = env.tempdir();
        out.push("out");

        // Prevent jobs from being run
        input.send(ProcessorInput::_Lock).unwrap();

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

            println!("{} {}", chr, priority);

            job = env.create_job("append-val.sh", Request::Web(req));
            input.send(ProcessorInput::Job(job, priority)).unwrap();
        }

        // Allow the processor to work
        input.send(ProcessorInput::_Unlock).unwrap();

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
    fn test_health_status() {
        let mut env = TestingEnv::new();

        let processor = Processor::new(
            1, env.hooks(), HashMap::new()
        ).unwrap();

        let input = processor.input();
        let mut out = env.tempdir();
        out.push("ok");

        // Queue a wait job
        let mut req = dummy_web_request();
        req.params.insert("env".into(), out.to_str().unwrap().to_string());
        let job = env.create_job("wait.sh", Request::Web(req));
        input.send(ProcessorInput::Job(job, 0)).unwrap();

        // Queue ten extra jobs
        let mut req;
        let mut job;
        for _ in 0..10 {
            req = Request::Web(dummy_web_request());
            job = env.create_job("example.sh", req);
            input.send(ProcessorInput::Job(job, 0)).unwrap();
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

        processor.stop().unwrap();

        // The file should not exist -- the first job removes it
        assert!(! out.exists());

        env.cleanup();
    }
}
