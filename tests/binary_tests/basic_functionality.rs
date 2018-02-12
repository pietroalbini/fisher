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

use std::fs::File;
use std::io::Read;

use reqwest;

use common::prelude::*;
use common::command::FisherCommand;


#[test]
fn fisher_executes_webhooks() {
    testing_env(|env| {
        let out = env.tempdir()?.join("out").to_str().unwrap().to_string();

        env.create_script("test.sh", &[
            r#"#!/bin/bash"#,
            r#"echo "executed" > "${TEST_OUTPUT_FILE}""#,
        ])?;

        let mut config = env.config();
        config.env.insert("TEST_OUTPUT_FILE".into(), out.clone());

        // Start the fisher binary with the sample configuration
        let mut fisher = FisherCommand::new(&config)?;

        // Call the webhook
        let addr = fisher.server_addr()?;
        let mut resp = reqwest::get(&format!("http://{}/hook/test.sh", addr))?;
        assert_eq!(resp.status().as_u16(), 200);
        assert_eq!(resp.text()?, r#"{"status":"ok"}"#);

        // Wait until the file is executed
        loop_timeout! {
            if let Ok(mut handle) = File::open(&out) {
                let mut buffer = String::new();
                handle.read_to_string(&mut buffer)?;

                // The file is actually empty when it's created, before bash
                // has time to write to it
                if buffer.len() == 0 {
                    continue;
                }

                assert_eq!(&buffer, "executed\n");
                break;
            }
        }

        fisher.stop()?;

        Ok(())
    });
}
