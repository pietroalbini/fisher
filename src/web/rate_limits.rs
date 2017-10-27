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

//! The Fisher rate limiter.
//!
//! The rate limiter takes an unusual approach for storing the users rate
//! limited. I designed this with a goal in mind: implement the thing without
//! any timer periodically decrementing the values, since that wakes up a
//! thread, wasting system resources.
//!
//! The rate limiter only stores the time when the number of counted requests
//! will reach 0, without storing the actual number of request. For each new
//! request `allowed_per_iterval / interval` seconds are added to that time.
//!
//! Then, to check if the user is rate limited the code simply subtracts the
//! current time to the limiting time, and if the delta is greater than
//! `interval` the user is rejected.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Instant, Duration};


#[derive(Debug)]
enum LimitStatus {
    Unlimited,
    ClearsAt(Instant, Duration),
}


#[derive(Debug)]
pub struct RateLimiter<Id: Hash + Eq + PartialEq> {
    data: HashMap<Id, LimitStatus>,
    incr_step: Duration,
    limit_after: Duration,
}

impl<Id: Hash + Eq + PartialEq> RateLimiter<Id> {
    pub fn new(allowed: u64, interval: u64) -> Self {
        RateLimiter {
            data: HashMap::new(),
            incr_step: Duration::from_millis(
                (interval as f64 / allowed as f64 * 1000.0) as u64
            ),
            limit_after: Duration::new(interval, 0),
        }
    }

    pub fn is_limited(&mut self, id: &Id) -> Option<Duration> {
        if let Some(status) = self.data.get_mut(id) {
            let mut reset = false;
            let result = match *status {
                LimitStatus::ClearsAt(ref start, ref duration) => {
                    let now = Instant::now();

                    let delta = now.duration_since(*start);
                    if delta > *duration {
                        // The limit fully expired
                        reset = true;
                        None
                    } else if *duration - delta > self.limit_after {
                        // The user is rate limited
                        Some(*duration - delta - self.limit_after)
                    } else {
                        // The user is not rate limited but is close
                        None
                    }
                },
                LimitStatus::Unlimited => None,
            };

            if reset {
                *status = LimitStatus::Unlimited;
            }

            result
        } else {
            // The user is not limited if he doesn't have an entry
            None
        }
    }

    pub fn increment(&mut self, id: Id) {
        let item = self.data.entry(id).or_insert(LimitStatus::Unlimited);

        if let LimitStatus::ClearsAt(start, duration) = *item {
            *item = LimitStatus::ClearsAt(start, duration + self.incr_step);
        } else {
            *item = LimitStatus::ClearsAt(Instant::now(), self.incr_step);
        }
    }
}


#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use super::RateLimiter;


    #[test]
    fn test_rate_limiter() {
        // Create a limiter that allows 10 requests in one second
        let mut limiter = RateLimiter::<u8>::new(10, 1);

        // Ensure ten requests are allowed
        for _ in 0..10 {
            limiter.increment(1);
            assert!(limiter.is_limited(&1).is_none());
        }

        // Ensure the eleventh is not
        limiter.increment(1);
        assert!(limiter.is_limited(&1).is_some());
    }


    #[test]
    #[ignore]
    fn test_rate_limiter_slow() {
        // Create a limiter that allows 2 requests a second
        let mut limiter = RateLimiter::<u8>::new(2, 1);

        // Do 2 requests a second for 2 seconds
        for _ in 0..4 {
            limiter.increment(1);
            assert!(limiter.is_limited(&1).is_none());

            thread::sleep(Duration::from_millis(500));
        }

        // Do 3 requests in less than a second, and ensure it doesn't validate
        for _ in 0..2 {
            limiter.increment(1);
            assert!(limiter.is_limited(&1).is_none());
        }
        limiter.increment(1);
        assert!(limiter.is_limited(&1).is_some());

        // Do a request after a second, and ensure it validates
        thread::sleep(Duration::from_secs(1));
        assert!(limiter.is_limited(&1).is_none());
    }
}
