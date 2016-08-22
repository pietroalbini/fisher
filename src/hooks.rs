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

use std::fs;
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::io::{BufReader, BufRead};

use regex::Regex;

use providers;
use processor;
use errors::FisherResult;


pub type VecProviders = Vec<providers::HookProvider>;
pub type Hooks = HashMap<String, Hook>;


lazy_static! {
    static ref HEADER_RE: Regex = Regex::new(
        r"## Fisher-([a-zA-Z]+): (\{.*\})"
    ).unwrap();
}


#[derive(Clone)]
pub struct Hook {
    name: String,
    exec: String,
    providers: VecProviders,
}

impl Hook {

    fn load(name: String, exec: String) -> FisherResult<Hook> {
        let providers = try!(Hook::load_providers(&exec));

        Ok(Hook {
            name: name,
            exec: exec,
            providers: providers,
        })
    }

    fn load_providers(file: &String) -> FisherResult<VecProviders> {
        let f = fs::File::open(file).unwrap();
        let reader = BufReader::new(f);

        let mut content;
        let mut line_number: u32 = 0;
        let mut result: VecProviders = vec![];
        for line in reader.lines() {
            line_number += 1;
            content = line.unwrap();

            // Just ignore everything after an empty line
            if content == "" {
                break;
            }

            // Capture every provider defined in the hook
            for cap in HEADER_RE.captures_iter(&content) {
                let name = cap.at(1).unwrap();
                let data = cap.at(2).unwrap();

                match providers::get(&name, &data) {
                    Ok(provider) => {
                        result.push(provider);
                    },
                    Err(mut error) => {
                        error.set_file(file.clone());
                        error.set_line(line_number);
                        return Err(error);
                    }
                }
            }
        }

        Ok(result)
    }

    pub fn validate(&self, req: &processor::Request) -> Option<JobHook> {
        if self.providers.len() > 0 {
            // Check every provider if they're present
            for provider in &self.providers {
                if provider.validate(&req) {
                    return Some(JobHook::new(
                        self.clone(), Some(provider.clone())
                    ));
                }
            }
            None
        } else {
            Some(JobHook::new(self.clone(), None))
        }
    }
}


#[derive(Clone)]
pub struct JobHook {
    hook: Hook,
    provider: Option<providers::HookProvider>,
}

impl JobHook {

    fn new(hook: Hook, provider: Option<providers::HookProvider>) -> Self {
        JobHook {
            hook: hook,
            provider: provider,
        }
    }

    pub fn name(&self) -> String {
        self.hook.name.clone()
    }

    pub fn exec(&self) -> String {
        self.hook.exec.clone()
    }

    pub fn env(&self, req: &processor::Request) -> HashMap<String, String> {
        // If there is a provider return the derived environment, else an
        // empty one
        if let Some(ref provider) = self.provider {
            provider.env(req)
        } else {
            HashMap::new()
        }
    }
}


pub fn collect<'a>(base: &String) -> FisherResult<Hooks> {
    let mut result = HashMap::new();

    for entry in try!(fs::read_dir(&base)) {
        let pathbuf = try!(entry).path();
        let path = pathbuf.as_path();

        // Check if the file is actually a file
        if ! path.is_file() {
            continue;
        }

        // Check if the file is executable and readable
        let mode = try!(path.metadata()).permissions().mode();
        if ! ((mode & 0o111) != 0 && (mode & 0o444) != 0) {
            // Skip files with wrong permissions
            continue
        }

        let name = path.file_stem().unwrap().to_str().unwrap().to_string();
        let exec = try!(fs::canonicalize(path)).to_str().unwrap().to_string();

        let hook = try!(Hook::load(name.clone(), exec));
        result.insert(name.clone(), hook);
    }

    Ok(result)
}
