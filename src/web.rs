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

use ansi_term::Colour;
use chan;
use nickel::{Nickel, MediaType, HttpRouter, Options};
use nickel::status::StatusCode;
use hyper::method::Method;

use processor::{Job, SenderChan};


pub struct WebAPI {
    stop_chan: Option<chan::Sender<()>>,
    sender_chan: Option<SenderChan>,
    stop: bool,
    hooks_names: Vec<String>,
}

impl WebAPI {

    pub fn new(hooks_names: Vec<String>) -> WebAPI {
        WebAPI {
            stop_chan: None,
            sender_chan: None,
            stop: false,
            hooks_names: hooks_names,
        }
    }

    pub fn listen(&mut self, bind: &str, sender: SenderChan) {
        // Store the sender channel
        self.sender_chan = Some(sender);

        // This is to fix lifetime issues with the thread below
        let bind = bind.to_string();

        let app = self.configure_nickel();

        // This channel is used so it's possible to stop the listener
        let (send_stop, recv_stop) = chan::sync(0);
        self.stop_chan = Some(send_stop);

        ::std::thread::spawn(move || {
            let bind: &str = &bind;
            match app.listen(bind) {
                Ok(listener) => {
                    println!("{} on {}",
                        Colour::Green.bold().paint("Web API listening"), bind
                    );

                    // This blocks until someone sends something to
                    // self.stop_chan
                    recv_stop.recv();

                    println!("Stopping web server...");

                    // Detach the webserver from the current thread, allowing
                    // the process to exit
                    listener.detach();
                },
                Err(error) => {
                    println!("{} on {}: {}",
                        Colour::Red.bold().paint(
                            "Failed to start the Web API"
                        ), bind, error
                    );
                    ::std::process::exit(1);
                }
            }
        });
    }

    pub fn stop(&mut self) -> bool {
        // Don't try to stop twice
        if self.stop {
            return true;
        }

        match self.stop_chan {
            Some(ref stop_chan) => {
                // Tell the thread to stop
                stop_chan.send(());

                self.stop = true;
                true
            },
            None => false,
        }
    }

    fn configure_nickel(&self) -> Nickel {
        let mut app = Nickel::new();

        // Disable the default message nickel prints on stdout
        app.options = Options::default().output_on_listen(false);

        for method in &[Method::Get, Method::Post] {
            // Make the used things owned
            let method = method.clone();
            let sender = self.sender_chan.clone().unwrap();
            let hooks_names = self.hooks_names.clone();

            // This middleware processes incoming hooks
            app.add_route(method, "/hook/:hook", middleware! { |req, mut res|
                res.set(MediaType::Json);

                let hook = req.param("hook").unwrap().to_string();

                // Ignore requests without a valid hook
                if hook == "".to_string() {
                    return res.next_middleware();
                }

                if hooks_names.contains(&hook) {
                    let job = Job::new(hook);

                    // Send the job to be processed
                    sender.send(Some(job));

                    r#"{"status":"queued"}"#
                } else {
                    r#"{"status":"not_found"}"#
                }
            });
        }

        // This middleware provides a basic Not found page
        app.utilize(middleware! { |_req, mut res|
            res.set(MediaType::Json);
            res.set(StatusCode::NotFound);

            r#"{"status":"not_found"}"#
        });

        app
    }

}
