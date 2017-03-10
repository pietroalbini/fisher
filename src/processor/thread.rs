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

use std::sync::{Arc, mpsc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::fmt;

use jobs::{Job, Context};
use hooks::Hooks;
use requests::Request;
use providers::StatusEvent;
use errors;

use super::scheduled_job::ScheduledJob;
use super::processor::ProcessorInput;


#[derive(Debug)]
enum ThreadInput {
    Process(ScheduledJob),
    StopSignal,
}


pub struct Thread {
    should_stop: bool,
    busy: Arc<AtomicBool>,

    handle: thread::JoinHandle<()>,
    input: mpsc::Sender<ThreadInput>,
}

impl Thread {

    pub fn new(processor_input: mpsc::Sender<ProcessorInput>,
               ctx: Arc<Context>, hooks: Arc<Hooks>) -> Thread {
        let (input_send, input_recv) = mpsc::channel();
        let busy = Arc::new(AtomicBool::new(false));

        let busy_inner = busy.clone();
        let handle = thread::spawn(move || {
            for input in input_recv.iter() {
                match input {
                    // A new job should be processed
                    ThreadInput::Process(job) => {
                        let result = job.job().process(&ctx);

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
                                    status_result = status_job.process(&ctx);

                                    if let Err(mut error) = status_result {
                                        error.set_hook(hp.hook.name().into());
                                        let _ = errors::print_err::<()>(Err(error));
                                    }
                                }
                            },
                            Err(mut error) => {
                                error.set_hook(job.job().hook_name().into());
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
    pub fn process(&self, job: ScheduledJob) -> Option<ScheduledJob> {
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
