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

use std::path::PathBuf;
use std::time::Duration;
use std::fs;

use chan;
use hyper::client as hyper;
use hyper::method::Method;

use super::WebAPI;
use hooks;
use hooks::tests::create_sample_hooks;
use processor::{HealthDetails, ProcessorInput};


pub struct TestInstance {
    inst: WebAPI,
    url: String,
    client: hyper::Client,

    tempdir: PathBuf,
    input_recv: chan::Receiver<ProcessorInput>,
}

impl TestInstance {

    pub fn new(health: bool) -> Self {
        // Create a new instance of WebAPI
        let tempdir = create_sample_hooks();
        let mut inst = WebAPI::new(hooks::collect(
            &tempdir.to_str().unwrap().to_string()
        ).unwrap());

        // Create the input channel
        let (input_send, input_recv) = chan::async();

        // Start the web server
        let addr = inst.listen("127.0.0.1:0", health, input_send).unwrap();

        // Create the HTTP client
        let url = format!("http://{}", addr);
        let client = hyper::Client::new();

        TestInstance {
            inst: inst,
            url: url,
            client: client,

            tempdir: tempdir,
            input_recv: input_recv,
        }
    }

    pub fn close(&mut self) {
        // Close the instance
        self.inst.stop();

        // Remove the directory
        fs::remove_dir_all(&self.tempdir).unwrap();
    }

    pub fn request(&mut self, method: Method, url: &str)
                   -> hyper::RequestBuilder {
        // Create the HTTP request
        self.client.request(method, &format!("{}{}", self.url, url))
    }

    pub fn processor_input(&self) -> Option<ProcessorInput> {
        let input_recv = &self.input_recv;

        // This returns Some only if there is something right now
        chan_select! {
            default => {
                return None;
            },
            input_recv.recv() -> input => {
                return Some(input.unwrap());
            },
        };
    }

    pub fn next_health(&self, details: HealthDetails) -> NextHealthCheck {
        let input_chan = self.input_recv.clone();
        let (result_send, result_recv) = chan::async();

        ::std::thread::spawn(move || {
            let input = input_chan.recv().unwrap();

            if let ProcessorInput::HealthStatus(ref sender) = input {
                // Send the HealthDetails we want
                sender.send(details);

                // Everything was OK
                result_send.send(None);
            } else {
                result_send.send(Some(
                    "Wrong kind of ProcessorInput received!".to_string()
                ));
            }
        });

        NextHealthCheck::new(result_recv)
    }
}


pub struct NextHealthCheck {
    result_recv: chan::Receiver<Option<String>>,
}

impl NextHealthCheck {

    fn new(result_recv: chan::Receiver<Option<String>>) -> Self {
        NextHealthCheck {
            result_recv: result_recv,
        }
    }

    pub fn check(&self) {
        let result_recv = &self.result_recv;

        let timeout = chan::after(Duration::from_secs(5));

        chan_select! {
            timeout.recv() => {
                panic!("No ProcessorInput received!");
            },
            result_recv.recv() -> result => {
                // Forward panics
                if let Some(message) = result.unwrap() {
                    panic!(message);
                }
            },
        };
    }
}
