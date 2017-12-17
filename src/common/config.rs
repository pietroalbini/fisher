// Copyright (C) 2017 Pietro Albini
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

//! This module contains the deserializable configuration structs used by
//! Fisher.

use std::collections::HashMap;
use std::str::FromStr;
use std::net::SocketAddr;
use std::fmt;
use std::result::Result as StdResult;

use serde::de::{Error as DeError, Visitor, Deserialize, Deserializer};

use common::prelude::*;
use utils;


macro_rules! default {
    ($struct:ident {$( $key:ident: $value:expr, )*}) => {
        impl Default for $struct {
            fn default() -> Self {
                $struct {
                    $( $key: $value ),*
                }
            }
        }
    }
}

macro_rules! default_fn {
    ($name:ident: $type:ty = $val:expr) => {
        fn $name() -> $type {
            $val
        }
    }
}


/// The Fisher configuration.
#[derive(Debug, Default, Deserialize)]
pub struct Config {
    /// Configuration for the built-in HTTP webhooks receiver.
    #[serde(default)]
    pub http: HttpConfig,
    /// Configuration for the scripts loading.
    #[serde(default)]
    pub scripts: ScriptsConfig,
    /// Configuration for running jobs.
    #[serde(default)]
    pub jobs: JobsConfig,
    /// Extra environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,
}


/// Configuration for the built-in HTTP webhooks receiver.
#[derive(Debug, Deserialize)]
pub struct HttpConfig {
    /// The number of proxies Fisher is behind.
    #[serde(rename="behind-proxies", default="default_behind_proxies")]
    pub behind_proxies: u8,
    /// The socket address to bind.
    #[serde(default="default_bind")]
    pub bind: SocketAddr,
    /// The rate limit for bad requests
    #[serde(rename="rate-limit", default)]
    pub rate_limit: RateLimitConfig,
    /// Enable or disable the health endpoint
    #[serde(rename="health-endpoint", default="default_health_endpoint")]
    pub health_endpoint: bool,
}

default_fn!(default_behind_proxies: u8 = 0);
default_fn!(default_bind: SocketAddr = "127.0.0.1:8000".parse().unwrap());
default_fn!(default_health_endpoint: bool = true);

default!(HttpConfig {
    behind_proxies: default_behind_proxies(),
    bind: default_bind(),
    rate_limit: RateLimitConfig::default(),
    health_endpoint: default_health_endpoint(),
});


/// Configuration for rate limiting.
#[derive(Debug)]
pub struct RateLimitConfig {
    /// The number of allowed requests in the interval.
    pub allowed: u64,
    /// The interval of time to consider.
    pub interval: utils::TimeString,
}

default!(RateLimitConfig {
    allowed: 10,
    interval: 60.into(),
});


impl FromStr for RateLimitConfig {
    type Err = Error;

    fn from_str(s: &str) -> Result<RateLimitConfig> {
        let slash_pos = s.char_indices()
            .filter(|ci| ci.1 == '/')
            .map(|ci| ci.0)
            .collect::<Vec<_>>();

        match slash_pos.len() {
            0 => Ok(RateLimitConfig {
                allowed: s.parse()?,
                interval: 60.into(),
            }),
            1 => {
                let (requests, interval) = s.split_at(slash_pos[0]);
                Ok(RateLimitConfig {
                    allowed: requests.parse()?,
                    interval: (&interval[1..]).parse()?,
                })
            },
            _ => Err(ErrorKind::InvalidRateLimitsConfig(s.into()).into()),
        }
    }
}

struct RateLimitConfigVisitor;

impl<'de> Visitor<'de> for RateLimitConfigVisitor {
    type Value = RateLimitConfig;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a number of seconds, a time string or a map")
    }

    fn visit_str<E: DeError>(self, s: &str) -> StdResult<RateLimitConfig, E> {
        match s.parse() {
            Ok(parsed) => Ok(parsed),
            Err(e) => Err(E::custom(e.to_string())),
        }
    }

    fn visit_i64<E>(self, num: i64) -> StdResult<RateLimitConfig, E> {
        Ok(RateLimitConfig {
            allowed: num as u64,
            interval: 60.into(),
        })
    }
}

impl<'de> Deserialize<'de> for RateLimitConfig {
    fn deserialize<D: Deserializer<'de>>(
        deserializer: D,
    ) -> StdResult<RateLimitConfig, D::Error> {
        deserializer.deserialize_any(RateLimitConfigVisitor)
    }
}


/// Configuration for running jobs.
#[derive(Debug, Deserialize)]
pub struct JobsConfig {
    /// The number of execution threads to use.
    #[serde(default = "default_threads")]
    pub threads: u16,
}

default_fn!(default_threads: u16 = 1);

default!(JobsConfig {
    threads: default_threads(),
});


/// Configuration for looking scripts up.
#[derive(Debug, Deserialize)]
pub struct ScriptsConfig {
    /// The path to search for hooks
    #[serde(default = "default_path")]
    pub path: String,
    /// Search subdirectories or not.
    #[serde(default = "default_subdirs")]
    pub subdirs: bool,
}

default_fn!(default_path: String = ".".into());
default_fn!(default_subdirs: bool = false);

default!(ScriptsConfig {
    path: default_path(),
    subdirs: default_subdirs(),
});
