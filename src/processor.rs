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

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use rustc_serialize::json::{Json, ToJson};
use chan;

use jobs::Job;
use hooks::Hooks;
use requests::Request;
use errors;


pub type SenderChan = chan::Sender<ProcessorInput>;


pub struct ProcessorManager {
    sender: Option<SenderChan>,
    stop_wait: Option<chan::Receiver<()>>,
}

impl ProcessorManager {

    pub fn new() -> ProcessorManager {
        ProcessorManager {
            sender: None,
            stop_wait: None,
        }
    }

    pub fn start(&mut self, max_threads: u16, hooks: Arc<Hooks>) {
        // This is used to retrieve the sender we want from the child thread
        let (sender_send, sender_recv) = chan::sync(0);

        // This is used by the thread to notify the processor it completed its
        // work, in order to block execution when stopping fisher
        let (stop_wait_send, stop_wait_recv) = chan::sync(0);

        ::std::thread::spawn(move || {
            let (mut processor, input) = Processor::new(max_threads, hooks);

            // Send the sender back to the parent thread
            sender_send.send(input);

            processor.run();

            // Notify ProcessorManager the thread did its work
            stop_wait_send.send(());
        });

        self.sender = Some(sender_recv.recv().unwrap());
        self.stop_wait = Some(stop_wait_recv);
    }

    pub fn stop(&self) {
        match self.sender {
            Some(ref sender) => {
                // Tell the processor to exit as soon as possible
                sender.send(ProcessorInput::StopSignal);

                // Wait until the processor did its work
                match self.stop_wait {
                    Some(ref stop_wait) => {
                        stop_wait.recv();
                    },
                    None => {},
                }
            },
            None => {},
        }
    }

    pub fn sender(&self) -> Option<SenderChan> {
        self.sender.clone()
    }
}


#[derive(Clone)]
pub enum ProcessorInput {
    StopSignal,
    Job(Job),
    HealthStatus(chan::Sender<HealthDetails>),
    JobEnded,
}


struct Processor {
    jobs: VecDeque<Job>,
    hooks: Arc<Hooks>,

    should_stop: bool,
    threads_count: u16,
    max_threads: u16,

    input_recv: chan::Receiver<ProcessorInput>,
    input_send: chan::Sender<ProcessorInput>,
}

impl Processor {

    pub fn new(max_threads: u16, hooks: Arc<Hooks>)
               -> (Processor, SenderChan) {
        // Create the channel for the input
        let (input_send, input_recv) = chan::async();

        let processor = Processor {
            jobs: VecDeque::new(),
            hooks: hooks,

            should_stop: false,
            threads_count: 0,
            max_threads: max_threads,

            input_recv: input_recv,
            input_send: input_send.clone(),
        };

        // Return both the processor and the input_send
        (processor, input_send)
    }

    pub fn run(&mut self) {
        loop {
            let input = self.input_recv.recv().unwrap();

            match input {
                ProcessorInput::StopSignal => {
                    // It's time to stop when no more jobs are left
                    self.should_stop = true;

                    // If no more jobs are left now, exit
                    if self.jobs.is_empty() {
                        break;
                    }
                },
                ProcessorInput::Job(job) => {
                    // Queue a new thread if there are too many threads
                    if self.threads_count >= self.max_threads {
                        self.jobs.push_back(job);
                    } else {
                        self.spawn_thread(job);
                    }
                },
                ProcessorInput::HealthStatus(return_to) => {
                    return_to.send(HealthDetails::of(&self));
                },
                ProcessorInput::JobEnded => {
                    self.threads_count -= 1;

                    match self.jobs.pop_front() {
                        Some(job) => {
                            self.spawn_thread(job);
                        },
                        None => {
                            if self.should_stop {
                                break;
                            }
                        },
                    };
                }
            }
        }
    }

    fn spawn_thread(&mut self, job: Job) {
        let input_send = self.input_send.clone();
        let hooks = self.hooks.clone();

        self.threads_count += 1;

        ::std::thread::spawn(move || {
            let result = job.process();

            // Display the error if there is one
            if let Err(mut error) = result {
                error.set_hook(job.hook_name().to_string());
                let _ = errors::print_err::<()>(Err(error));
            } else {
                let output = result.unwrap();
                let req: Request = output.into();

                for (hook_name, hook) in hooks.iter() {
                    // Validate the request only on the Status providers
                    let opt_provider = hook.validate_provider("Status", &req);

                    if let Some(provider) = opt_provider {
                        let status_job = Job::new(
                            hook.clone(), Some(provider), req.clone()
                        );

                        // Process the new status job
                        let result = status_job.process();
                        if let Err(mut error) = result {
                            error.set_hook(hook_name.to_string());
                            let _ = errors::print_err::<()>(Err(error));
                        }
                    }
                }
            }

            // Notify the end of this thread
            input_send.send(ProcessorInput::JobEnded);
        });
    }

}


#[derive(Clone, Debug)]
pub struct HealthDetails {
    pub queue_size: usize,
    pub active_jobs: u16,
}

impl HealthDetails {

    fn of(processor: &Processor) -> Self {
        // Collect some details of that processor
        let queue_size = processor.jobs.len();
        let active_jobs = processor.threads_count;

        HealthDetails {
            queue_size: queue_size,
            active_jobs: active_jobs,
        }
    }
}

impl ToJson for HealthDetails {

    fn to_json(&self) -> Json {
        let mut map = BTreeMap::new();
        map.insert("queue_size".to_string(), self.queue_size.to_json());
        map.insert("active_jobs".to_string(), self.active_jobs.to_json());

        Json::Object(map)
    }
}
