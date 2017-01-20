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

use std::net::IpAddr;

use requests::Request;
use errors::{FisherResult, ErrorKind};
use utils;


#[derive(Debug, PartialEq, Clone)]
pub struct ProxySupport {
    behind: Option<u8>,
}

impl ProxySupport {

    pub fn new(behind: Option<u8>) -> Self {
        ProxySupport {
            behind: behind,
        }
    }

    pub fn source_ip(&self, req: &Request) -> FisherResult<IpAddr> {
        let req = req.web()?;
        let original = req.source;

        // Return the original IP if the proxy support is disabled
        if self.behind.is_none() {
            return Ok(original);
        }

        // Parse the X-Forwarded-For header
        let mut forwarded_ips = utils::parse_forwarded_for(&req.headers)?;

        // Return an error if there was no header
        if forwarded_ips.is_empty() {
            return Err(ErrorKind::NotBehindProxy.into());
        }

        // This puts the closest proxies before
        forwarded_ips.reverse();

        // Return the correct IP if there are enough proxies, or an error if
        // there are too few
        let index = (self.behind.unwrap() - 1) as usize;
        if let Some(ip) = forwarded_ips.get(index) {
            Ok(*ip)
        } else {
            Err(ErrorKind::NotBehindProxy.into())
        }
    }

    pub fn fix_request(&self, req: &mut Request) -> FisherResult<()> {
        let fixed_ip = self.source_ip(req)?;

        if let Request::Web(ref mut req) = *req {
            req.source = fixed_ip;
            Ok(())
        } else {
            Err(ErrorKind::WrongRequestKind.into())
        }
    }
}


#[cfg(test)]
mod tests {
    use std::net::IpAddr;
    use std::str::FromStr;

    use utils::testing::*;
    use errors::ErrorKind;
    use requests::Request;

    use super::ProxySupport;


    // This macro creates a dummy request with a different source IP and,
    // optionally, a custom X-Forwarded-For
    macro_rules! req {
        () => {{
            let mut req = dummy_web_request();
            req.source = IpAddr::from_str("127.1.1.1").unwrap();
            Request::Web(req)
        }};
        ($fwd_for:expr) => {{
            let mut req = req!();
            if let Request::Web(ref mut inner) = req {
                inner.headers.insert(
                    "X-Forwarded-For".into(),
                    $fwd_for.into()
                );
            }
            req
        }};
    }


    #[test]
    fn test_creation() {
        // Create a new disabled ProxySupport instance
        let proxy = ProxySupport::new(None);
        assert_eq!(proxy.behind, None);

        // Create a new enabled ProxySupport instance
        let proxy = ProxySupport::new(Some(1));
        assert_eq!(proxy.behind, Some(1));
    }


    #[test]
    fn test_source_ip() {
        macro_rules! assert_ip {
            ($proxy:expr, $req:expr, $expected:expr) => {{
                assert_eq!(
                    $proxy.source_ip(&$req).unwrap(),
                    IpAddr::from_str($expected).unwrap()
                );
            }};
        }

        // Test with a disabled proxy support
        let p = ProxySupport::new(None);
        assert_ip!(p, req!(), "127.1.1.1");
        assert_ip!(p, req!("127.2.2.2"), "127.1.1.1");
        assert_ip!(p, req!("127.3.3.3, 127.2.2.2"), "127.1.1.1");
        assert_ip!(p, req!("invalid"), "127.1.1.1");

        // Test with an enabled proxy support with one proxy
        let p = ProxySupport::new(Some(1));
        assert_err!(p.source_ip(&req!()), ErrorKind::NotBehindProxy);
        assert_ip!(p, req!("127.2.2.2"), "127.2.2.2");
        assert_ip!(p, req!("127.3.3.3, 127.2.2.2"), "127.2.2.2");
        assert_err!(
            p.source_ip(&req!("invalid")),
            ErrorKind::AddrParseError(..)
        );

        // Test with an enabled proxy support with two proxies
        let p = ProxySupport::new(Some(2));
        assert_err!(p.source_ip(&req!()), ErrorKind::NotBehindProxy);
        assert_err!(
            p.source_ip(&req!("127.2.2.2")),
            ErrorKind::NotBehindProxy
        );
        assert_ip!(p, req!("127.3.3.3, 127.2.2.2"), "127.3.3.3");
        assert_err!(
            p.source_ip(&req!("invalid")),
            ErrorKind::AddrParseError(..)
        );
    }


    #[test]
    fn test_fix_request() {
        let proxy = ProxySupport::new(Some(1));
        let mut req = req!("127.2.2.2");

        assert_eq!(
            req.web().unwrap().source,
            IpAddr::from_str("127.1.1.1").unwrap()
        );
        proxy.fix_request(&mut req).unwrap();
        assert_eq!(
            req.web().unwrap().source,
            IpAddr::from_str("127.2.2.2").unwrap()
        );
    }
}
