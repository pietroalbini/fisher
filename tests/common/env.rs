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

use std::cell::RefCell;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use tempdir::TempDir;

use common::prelude::*;


pub struct TestingEnv {
    tempdirs: RefCell<Vec<PathBuf>>,
    scripts_dir: PathBuf,
}

impl TestingEnv {
    fn new() -> Result<Self> {
        let scripts_dir = TempDir::new("fisher-integration")?.into_path();

        Ok(TestingEnv {
            tempdirs: RefCell::new(vec![scripts_dir.clone()]),
            scripts_dir,
        })
    }

    pub fn scripts_path(&self) -> &Path {
        &self.scripts_dir
    }

    pub fn tempdir(&self) -> Result<PathBuf> {
        let dir = TempDir::new("fisher-integration")?.into_path();
        self.tempdirs.borrow_mut().push(dir.clone());
        Ok(dir)
    }

    pub fn create_script(&self, name: &str, content: &[&str]) -> Result<()> {
        let path = self.scripts_dir.join(name);
        let mut file = OpenOptions::new()
            .write(true)
            .mode(0o755)
            .create(true)
            .open(&path)?;
        writeln!(file, "{}", content.join("\n"))?;
        Ok(())
    }

    pub fn config(&self) -> Config {
        Config::new(self)
    }

    fn cleanup(self) -> Result<()> {
        for dir in self.tempdirs.borrow().iter() {
            fs::remove_dir_all(dir)?;
        }

        Ok(())
    }
}


pub fn testing_env<F: Fn(&mut TestingEnv) -> Result<()>>(f: F) {
    let mut env = TestingEnv::new().unwrap();

    let result = f(&mut env);
    env.cleanup().unwrap();
    result.unwrap();
}
