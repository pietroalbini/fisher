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

use rustc_serialize::json::{Json, ToJson};
use chan;

use jobs::Job;
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

    pub fn start(&mut self, max_threads: u16) {
        // This is used to retrieve the sender we want from the child thread
        let (sender_send, sender_recv) = chan::sync(0);

        // This is used by the thread to notify the processor it completed its
        // work, in order to block execution when stopping fisher
        let (stop_wait_send, stop_wait_recv) = chan::sync(0);

        ::std::thread::spawn(move || {
            let (mut processor, input) = Processor::new(max_threads);

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
}


struct Processor {
    jobs: VecDeque<Job>,

    should_stop: bool,
    threads_count: u16,
    max_threads: u16,

    input: chan::Receiver<ProcessorInput>,
    thread_end: Option<chan::Sender<()>>,
}

impl Processor {

    pub fn new(max_threads: u16) -> (Processor, SenderChan) {
        // Create the channel for the input
        let (input_send, input_recv) = chan::async();

        let processor = Processor {
            jobs: VecDeque::new(),

            should_stop: false,
            threads_count: 0,
            max_threads: max_threads,

            input: input_recv,
            thread_end: None,
        };

        // Return both the processor and the input_send
        (processor, input_send)
    }

    pub fn run(&mut self) {
        // This channel will be notified when a thread ends
        let (thread_end_send, thread_end_recv) = chan::async();
        self.thread_end = Some(thread_end_send);

        let input_chan = self.input.clone();

        loop {
            chan_select! {
                // This means a new job was received, or it's time to stop
                input_chan.recv() -> input => {
                    let input = input.unwrap();

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
                    };
                },
                // This means a thread exited
                thread_end_recv.recv() => {
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
                },
            }
        }
    }

    fn spawn_thread(&mut self, job: Job) {
        let thread_end = self.thread_end.clone().unwrap();

        self.threads_count += 1;

        ::std::thread::spawn(move || {
            let result = job.process();

            // Display the error if there is one
            if let Err(mut error) = result {
                error.set_hook(job.hook_name().to_string());
                let _ = errors::print_err::<()>(Err(error));
            }

            // Notify the end of this thread
            thread_end.send(());
        });
    }

}


#[derive(Clone)]
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
