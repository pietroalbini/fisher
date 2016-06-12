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
use std::os::unix::fs::PermissionsExt;
use std::io::{BufReader, BufRead};

use regex::Regex;

use providers;
use errors::FisherError;


lazy_static! {
    static ref HEADER_RE: Regex = Regex::new(
        r"## Fisher-([a-zA-Z]+): (\{.*\})"
    ).unwrap();
}


pub struct Hook {
    pub name: String,
    pub exec: String,
    pub providers: Vec<Box<providers::HooksProvider>>,
}

impl Hook {

    fn load(name: String, exec: String) -> Hook {
        let mut hook = Hook {
            name: name,
            exec: exec,
            providers: vec![],
        };
        hook.load_providers();
        hook
    }

    fn load_providers(&mut self) {
        let mut f = fs::File::open(&self.exec).unwrap();
        let mut reader = BufReader::new(f);

        let mut content;
        for line in reader.lines() {
            content = line.unwrap();

            // Just ignore everything after an empty line
            if content == "" {
                break;
            }

            // Capture every provider defined in the hook
            for cap in HEADER_RE.captures_iter(&content) {
                let name = cap.at(1).unwrap();
                let data = cap.at(2).unwrap();

                if let Some(provider) = providers::by_name(&name, &data) {
                    self.providers.push(provider);
                } else {
                    println!("Nope!");
                }
            }
        }
    }

}


pub fn collect<'a>(base: &String) -> Result<Vec<Hook>, FisherError> {
    let metadata = fs::metadata(&base);
    if metadata.is_err() {
        return Err(FisherError::PathNotFound(base.clone()));
    }
    let metadata = metadata.unwrap();

    if ! metadata.is_dir() {
        return Err(FisherError::PathNotADirectory(base.clone()));
    }

    let mut result = Vec::new();

    for entry in fs::read_dir(&base).unwrap() {
        let pathbuf = entry.unwrap().path();
        let path = pathbuf.as_path();

        // Check if the file is actually a file
        if ! path.is_file() {
            continue;
        }

        // Check if the file is executable and readable
        let mode = path.metadata().unwrap().permissions().mode();
        if ! ((mode & 0o111) != 0 && (mode & 0o444) != 0) {
            // Skip files with wrong permissions
            continue
        }

        let name = path.file_stem().unwrap().to_str().unwrap().to_string();
        let exec = path.to_str().unwrap().to_string();

        let hook = Hook::load(name, exec);
        result.push(hook);
    }

    Ok(result)
}
