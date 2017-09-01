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

use std::env;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::io;
use std::path;
use std::sync::Mutex;

use rand::{self, Rng};
#[cfg(test)] use rand::{SeedableRng};

use common::prelude::*;


lazy_static! {
    static ref CREATOR: Mutex<TempDirCreator> = {
        match TempDirCreator::new("fisher") {
            Ok(creator) => Mutex::new(creator),
            Err(error) => {
                error.pretty_print();
                ::std::process::exit(1);
            },
        }
    };
}


struct TempDirCreator {
    prefix: String,
    rng: rand::StdRng,
}

impl TempDirCreator {

    fn new(prefix: &str) -> Result<Self> {
        // This might fail because it's not able to seed
        let rng = rand::StdRng::new()?;

        Ok(TempDirCreator {
            prefix: prefix.to_string(),
            rng: rng,
        })
    }

    fn create(&mut self) -> Result<path::PathBuf> {
        // The OS's base temp directory
        let base = env::temp_dir();

        // Create a randomized temp directory
        loop {
            // Generate the random suffix
            let suffix: String = self.rng.gen_ascii_chars().take(10).collect();

            let mut path = base.clone();
            path.push(format!("{}-{}", self.prefix, suffix));

            // Be sure to set the 0700 permissions on the new directory
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700);

            // Create the path, and handle the errors
            if let Err(error) = builder.create(&path) {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    // If the directory already exists retry
                    continue;
                } else {
                    // Return the converted error
                    return Err(::std::convert::From::from(error));
                }
            }

            return Ok(path);
        };
    }

    #[cfg(test)]  // This is used only during tests
    fn seed(&mut self, seed: &[usize]) {
        self.rng.reseed(seed);
    }
}


pub fn create_temp_dir() -> Result<path::PathBuf> {
    let mut creator = CREATOR.lock().unwrap();
    creator.create()
}


#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::env;
    use std::path;
    use std::fs;

    use rand;
    use rand::Rng;

    use super::TempDirCreator;


    #[test]
    fn test_temp_dir_creation() {
        let prefix = generate_prefix();

        // Load the base path
        let mut base = path::PathBuf::new();
        base.push(env::temp_dir());

        // Due to the seed, the first expected directory is dmcNjFlIHW
        let mut expected1 = base.clone();
        expected1.push(&format!("{}-dmcNjFlIHW", prefix));

        // Due to the seed, the second expected directory is Dw7ryRxsdw
        let mut expected2 = base.clone();
        expected2.push(&format!("{}-Dw7ryRxsdw", prefix));

        // Create a new TempDirCreator
        let mut creator = TempDirCreator::new(&prefix).unwrap();

        // Seed the creator to a known state
        creator.seed(&[1]);

        // Try to create a temp directory, and remove it
        let created = creator.create().unwrap();
        assert!(created == expected1);

        // Check if its metadata is correct
        let metadata = fs::metadata(&expected1).unwrap();
        assert!(metadata.is_dir());
        assert!(metadata.permissions().mode() == 0o700);

        // Then, the RNG is reseeded to simulate choosing a directory name
        // which already exists
        creator.seed(&[1]);

        // This should be expected1, but since it already exists the output
        // must be expected2
        let created = creator.create().unwrap();
        assert!(created == expected2);

        // Delete the two directories
        fs::remove_dir_all(expected1).unwrap();
        fs::remove_dir_all(expected2).unwrap();
    }


    fn generate_prefix() -> String {
        // Use this thread's random number generator
        let mut rng = rand::thread_rng();

        let prefix: String = rng.gen_ascii_chars().take(10).collect();
        format!("fisher-tests-{}", prefix)
    }
}
