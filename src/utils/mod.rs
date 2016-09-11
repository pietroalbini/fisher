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

mod tempdir;
mod copy_to_clone;
mod net;

#[cfg(test)]
mod parse_env;

#[cfg(test)]
pub mod testing;


pub use utils::tempdir::create_temp_dir;
pub use utils::copy_to_clone::CopyToClone;
pub use utils::net::parse_forwarded_for;

#[cfg(test)]
pub use utils::parse_env::parse_env;
