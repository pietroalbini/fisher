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


pub trait HooksProvider {}


// The "Standalone" provider is not tied to any external service, and requires
// a valid secret key when receiving an hook

#[derive(RustcDecodable, RustcEncodable)]
pub struct StandaloneProvider {
    secret: String,
}

impl HooksProvider for StandaloneProvider {}


// This is a list of all the providers currently supported by Fisher

pub enum Provider {
    StandaloneProvider,
}

impl Provider {

    fn by_name(name: &str) -> Option<Provider> {
        match name {
            "Standalone" => Some(Provider::StandaloneProvider),
            _ => None,
        }
    }

}
