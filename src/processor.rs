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
use errors::FisherResult;
use logger::{Logger, LogEvent};

#[derive(Clone)]
pub enum ProcessorInput {
    Job(Job),
    HealthStatus(mpsc::Sender<HealthDetails>),

    _StopSignal,
    _JobEnded,
}


#[derive(Debug)]
pub struct Processor {
    input: mpsc::Sender<ProcessorInput>,
    wait: mpsc::Receiver<()>,
    logger: Logger,
}

impl Processor {

    pub fn new(max_threads: u16, hooks: Arc<Hooks>, logger: Logger) -> FisherResult<Self> {
        // Retrieve wanted information from the spawned thread
        let (input_send, input_recv) = mpsc::sync_channel(0);
        let (wait_send, wait_recv) = mpsc::channel();

        let logger_clone = logger.clone();
        ::std::thread::spawn(move || {
            let inner = InnerProcessor::new(
                max_threads, hooks, logger_clone,
            );
            input_send.send(inner.input()).unwrap();

            inner.run().unwrap();

            // Notify the main thread this exited
            wait_send.send(()).unwrap();
        });

        Ok(Processor {
            input: input_recv.recv()?,
            wait: wait_recv,
            logger: logger,
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
    logger: Logger,

    should_stop: bool,
    queue: VecDeque<Job>,
    threads: Vec<Thread>,

    input_send: mpsc::Sender<ProcessorInput>,
    input_recv: mpsc::Receiver<ProcessorInput>,
}

impl InnerProcessor {

    fn new(max_threads: u16, hooks: Arc<Hooks>, logger: Logger) -> Self {
        let (input_send, input_recv) = mpsc::channel();

        InnerProcessor {
            max_threads: max_threads,
            hooks: hooks,
            logger: logger,

            should_stop: false,
            queue: VecDeque::new(),
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

        while let Ok(input) = self.input_recv.recv() {
            match input {

                ProcessorInput::Job(job) => {
                    self.queue.push_back(job);
                    self.run_jobs();
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
            self.logger.clone(), self.input_send.clone(), self.hooks.clone()
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
        // Here there is a loop so if for some reason there are multiple
        // threads available and there are enough elements in the queue,
        // all of them are processed
        'main: loop {
            if let Some(mut job) = self.queue.pop_front() {
                // Try to run the job in a thread
                for thread in &self.threads {
                    // The process() method returns Some(Job) if
                    // *IT'S BUSY* working on another job
                    if let Some(j) = thread.process(job) {
                        job = j;
                    } else {
                        continue 'main;
                    }
                }
                self.queue.push_front(job);
            }
            break;
        }
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

    pub fn new(logger: Logger, processor_input: mpsc::Sender<ProcessorInput>,
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
                                        logger.log(LogEvent::Error(error));
                                    }
                                }
                            },
                            Err(mut error) => {
                                error.set_hook(job.hook_name().into());
                                logger.log(LogEvent::Error(error));
                            }
                        }

                        busy_inner.store(false, Ordering::Relaxed);
                        processor_input.send(
                            ProcessorInput::_JobEnded
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

    use super::{Processor, ProcessorInput};


    #[test]
    fn test_processor_starting() {
        let env = TestingEnv::new();

        let processor = Processor::new(1, env.hooks(), env.logger()).unwrap();
        processor.stop().unwrap();

        env.cleanup();
    }


    #[test]
    fn test_processor_clean_stop() {
        let mut env = TestingEnv::new();

        let processor = Processor::new(1, env.hooks(), env.logger()).unwrap();

        // Prepare a request
        let mut out = env.tempdir();
        out.push("ok");

        let mut req = dummy_web_request();
        req.params.insert("env".into(), out.to_str().unwrap().to_string());

        // Queue a dummy job
        let job = env.create_job("long", Request::Web(req));
        processor.input().send(ProcessorInput::Job(job)).unwrap();

        // Exit immediately -- this forces the processor to wait since the job
        // sleeps for half a second
        processor.stop().unwrap();

        // Check if the job was not killed
        assert!(out.exists(), "job was killed before it completed");

        env.cleanup();
    }


    fn run_multiple_append(threads: u16) -> String {
        let mut env = TestingEnv::new();

        let processor = Processor::new(threads, env.hooks(), env.logger()).unwrap();

        let input = processor.input();
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

        let processor = Processor::new(1, env.hooks(), env.logger()).unwrap();

        let input = processor.input();
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

        processor.stop().unwrap();

        // The file should not exist -- the first job removes it
        assert!(! out.exists());

        env.cleanup();
    }
}
