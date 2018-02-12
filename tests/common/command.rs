// Copyright (C) 2018 Pietro Albini
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

use std::env;
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::process;

use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use regex::Regex;

use common::config::Config;
use common::prelude::*;


lazy_static! {
    static ref ADDR_RE: Regex = Regex::new(r"127\.0\.0\.1:[0-9]+").unwrap();
}


fn binaries_path() -> Result<PathBuf> {
    let mut current = env::current_exe()?;
    current.pop();

    if current.ends_with("deps") {
        current.pop();
    }

    Ok(current)
}


#[allow(dead_code)]
pub enum Stream {
    Stdout,
    Stderr,
}


pub struct Command {
    child: process::Child,
    stdout: BufReader<process::ChildStdout>,
    stderr: BufReader<process::ChildStderr>,
}

impl Command {
    pub fn new(binary: &str, args: &[&str]) -> Result<Self> {
        let mut child = process::Command::new(binaries_path()?.join(binary))
            .args(args)
            .stdout(process::Stdio::piped())
            .stderr(process::Stdio::piped())
            .spawn()?;

        Ok(Command {
            stdout: BufReader::new(child.stdout.take().unwrap()),
            stderr: BufReader::new(child.stderr.take().unwrap()),
            child,
        })
    }

    pub fn capture_line(
        &mut self,
        content: &str,
        stream: Stream,
    ) -> Result<String> {
        let reader = match stream {
            Stream::Stdout => &mut self.stdout as &mut BufRead,
            Stream::Stderr => &mut self.stderr as &mut BufRead,
        };

        let mut buffer = String::new();
        loop {
            buffer.clear();
            reader.read_line(&mut buffer)?;

            if buffer.contains(content) {
                break;
            }
        }

        buffer.shrink_to_fit();
        Ok(buffer)
    }

    pub fn signal(&mut self, signal: Signal) -> Result<()> {
        kill(Pid::from_raw(self.child.id() as i32), signal)?;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.signal(Signal::SIGTERM)?;
        self.wait()
    }

    pub fn wait(&mut self) -> Result<()> {
        self.child.wait()?;
        Ok(())
    }
}


pub struct FisherCommand {
    inner: Command,
}

impl FisherCommand {
    pub fn new(config: &Config) -> Result<Self> {
        Ok(FisherCommand {
            inner: Command::new("fisher", &[
                config.save()?.to_str().unwrap(),
            ])?,
        })
    }

    pub fn server_addr(&mut self) -> Result<SocketAddr> {
        let line = self.capture_line("listening", Stream::Stdout)?;
        let captures = ADDR_RE.captures(&line).unwrap();
        println!("{:?}", captures);
        Ok((&captures[0]).parse()?)
    }
}

impl Deref for FisherCommand {
    type Target = Command;

    fn deref(&self) -> &Command {
        &self.inner
    }
}

impl DerefMut for FisherCommand {
    fn deref_mut(&mut self) -> &mut Command {
        &mut self.inner
    }
}
