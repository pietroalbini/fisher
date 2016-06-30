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

use nickel::{Nickel, Request, Response, MiddlewareResult, MediaType,
             HttpRouter, Options};
use nickel::status::StatusCode;


fn handle_queue<'mw>(req: &mut Request, mut res: Response<'mw>)
                     -> MiddlewareResult<'mw> {
    let hook = req.param("hook").unwrap();

    // Ignore requests without a valid hook
    if hook == "" {
        return res.next_middleware();
    }

    println!("Fake processing hook {}...", hook);

    res.set(MediaType::Json);
    res.send(r#"{"status":"queued"}"#)
}


fn not_found<'mw>(req: &mut Request, mut res: Response<'mw>)
                  -> MiddlewareResult<'mw> {
    res.set(MediaType::Json);
    res.set(StatusCode::NotFound);
    res.send(r#"{"status":"not_found"}"#)
}


pub fn create_app() -> Nickel {
    let mut app = Nickel::new();

    // Disable the default message nickel prints on stdout
    app.options = Options::default().output_on_listen(false);

    app.get("/hook/:hook", handle_queue);
    app.post("/hook/:hook", handle_queue);

    app.utilize(not_found);

    app
}
