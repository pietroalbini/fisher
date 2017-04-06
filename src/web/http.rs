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

use std::net::{SocketAddr, TcpStream, Shutdown};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use regex::{self, Regex};
use tiny_http::{self, Method};

use errors::FisherResult;
use requests::Request;
use web::responses::Response;
use web::proxies::ProxySupport;


pub type RequestHandler<App> = Box<
    fn(&App, &Request, Vec<String>) -> Response
>;


struct Route {
    method: Method,
    regex: Regex,
}

impl Route {

    fn new(method: Method, url: &str) -> Self {
        let regex = Self::regex_from_url(url);

        Route {
            method: method,
            regex: Regex::new(&regex).unwrap(),
        }
    }

    fn regex_from_url(url: &str) -> String {
        let mut result = "^".to_string();

        let mut first = true;
        for part in url.split('/') {
            // Add a slash at the start
            if first {
                first = false;
            } else {
                result.push('/');
            }

            if part == "?" {
                result.push_str(r"([a-zA-Z0-9\./_-]+)");
            } else {
                result.push_str(&regex::quote(part));
            }
        }

        result.push_str(r"(\?.*)?$");
        result
    }

    fn matches(&self, method: &Method, url: &str) -> Option<Vec<String>> {
        // Methods should match
        if *method != self.method {
            return None;
        }

        match self.regex.captures(url) {
            Some(captures) => {
                Some(
                    captures.iter().skip(1)
                            .filter(|x| x.is_some())
                            .map(|x| x.unwrap().to_string())
                            .collect()
                )
            },
            None => None,
        }
    }
}


struct Handler<App: Send + Sync + 'static> {
    handler: RequestHandler<App>,
    route: Route,
}

impl<App: Send + Sync + 'static> Handler<App> {

    fn new(handler: RequestHandler<App>, route: Route) -> Self {
        Handler {
            handler: handler,
            route: route,
        }
    }

    fn matches(&self, method: &Method, url: &str) -> Option<Vec<String>> {
        self.route.matches(method, url)
    }

    fn call(&self, app: &App, req: &Request, args: Vec<String>) -> Response {
        (self.handler)(app, req, args)
    }
}


pub struct HttpServer<App: Send + Sync + 'static> {
    app: Arc<App>,
    handlers: Arc<Mutex<Vec<Handler<App>>>>,
    proxy_support: Arc<ProxySupport>,

    should_stop: Arc<AtomicBool>,

    listening_to: Option<SocketAddr>,
    stop_wait: Option<mpsc::Receiver<()>>,
}

impl<App: Send + Sync + 'static> HttpServer<App> {

    pub fn new(app: App, proxies_count: u8) -> Self {
        HttpServer {
            app: Arc::new(app),
            handlers: Arc::new(Mutex::new(Vec::new())),
            proxy_support: Arc::new(ProxySupport::new(proxies_count)),

            should_stop: Arc::new(AtomicBool::new(false)),

            listening_to: None,
            stop_wait: None,
        }
    }

    pub fn add_route(&mut self, method: Method, url: &str,
                     handler: RequestHandler<App>) {
        let route = Route::new(method, url);
        self.handlers.try_lock().unwrap().push(
            Handler::new(handler, route)
        );
    }

    pub fn listen(&mut self, bind: &str) -> FisherResult<SocketAddr> {
        macro_rules! header {
            ($value:expr) => {
                $value.parse::<tiny_http::Header>().unwrap()
            };
        }

        // This will move to the thread, and the server will be stopped when
        // the thread exits
        let server = tiny_http::Server::http(bind.parse::<SocketAddr>()?)?;

        // Store the server address into the struct
        self.listening_to = Some(server.server_addr());

        let (stop_send, stop_recv) = mpsc::channel();
        self.stop_wait = Some(stop_recv);

        let app = self.app.clone();
        let handlers_arc = self.handlers.clone();
        let proxy_support = self.proxy_support.clone();
        let should_stop = self.should_stop.clone();
        thread::spawn(move || {
            // Get a reference to the handlers
            let handlers = &*handlers_arc.lock().unwrap();

            // Prepare some headers which will be sent everytime
            let server_header = header!(
                format!("Server: Fisher/{}", env!("CARGO_PKG_VERSION"))
            );
            let content_type = header!("Content-Type: application/json");

            let ignored_method = Method::NonStandard(
                "X_FISHER_IGNORE_THIS".parse().unwrap()
            );

            for mut request in server.incoming_requests() {
                // Don't accept any request anymore
                if should_stop.load(Ordering::Relaxed) {
                    break;
                }

                // Convert the request to a Fisher request
                let mut req = Request::Web((&mut request).into());

                let response = (|| {
                    if *request.method() == ignored_method {
                        // This request comes with the non-standard method used
                        // to shut the server down -- no client should be using
                        // it
                        Response::Forbidden
                    } else if let Err(e) = proxy_support.fix_request(&mut req) {
                        Response::BadRequest(e)
                    } else {
                        let method = request.method();
                        let url = request.url();

                        for handler in handlers {
                            if let Some(args) = handler.matches(method, url) {
                                return handler.call(&app, &req, args);
                            }
                        }

                        Response::NotFound
                    }
                })();

                let mut tiny_response = tiny_http::Response::from_data(
                    response.json().into_bytes()
                ).with_status_code(response.status());

                tiny_response.add_header(server_header.clone());
                tiny_response.add_header(content_type.clone());

                let _ = request.respond(tiny_response);
            }

            stop_send.send(()).unwrap();
        });

        Ok(self.listening_to.unwrap())
    }

    pub fn stop(&mut self) -> bool {
        if self.stop_wait.is_some() {
            // Tell the server to stop
            self.should_stop.store(true, Ordering::Relaxed);

            // Send an HTTP request to force stopping the server
            match TcpStream::connect(self.listening_to.unwrap()) {
                Ok(mut conn) => {
                    (writeln!(conn,
                        "X_FISHER_IGNORE_THIS / HTTP/1.0\r\n\r\n"
                    )).unwrap();
                    conn.shutdown(Shutdown::Both).unwrap();
                },
                Err(..) => {
                    return false;
                },
            }

            if let Some(ref stop_wait) = self.stop_wait {
                // Wait for the http server to stop
                stop_wait.recv().unwrap();
            } else {
                unreachable!();
            }

            self.stop_wait = None;
            self.listening_to = None;

            true
        } else{
            false
        }
    }
}


#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tiny_http::Method;
    use hyper;
    use hyper::status::StatusCode;

    use requests::Request;
    use web::responses::Response;
    use utils::testing::*;
    use super::{Route, Handler, HttpServer};


    struct DummyData(Vec<String>);

    fn dummy_handler_fn(data: &DummyData, _req: &Request, args: Vec<String>)
                        -> Response {
        if data.0 == args {
           Response::Ok
        } else {
           Response::Forbidden
        }
    }

    fn dummy_handler() -> Handler<DummyData> {
        let route = Route::new(Method::Get, "/?");
        Handler::new(Box::new(dummy_handler_fn), route)
    }


    #[test]
    fn test_route_regex_from_url() {
        macro_rules! conv { ($inp:expr) => { Route::regex_from_url($inp) }};

        assert_eq!(conv!("/"), r"^/(\?.*)?$");
        assert_eq!(conv!("/."), r"^/\.(\?.*)?$");
        assert_eq!(conv!("/test"), r"^/test(\?.*)?$");
        assert_eq!(conv!("/?"), r"^/([a-zA-Z0-9\./_-]+)(\?.*)?$");
        assert_eq!(conv!("/test/?"), r"^/test/([a-zA-Z0-9\./_-]+)(\?.*)?$");
        assert_eq!(
            conv!("/?/?/test"),
            r"^/([a-zA-Z0-9\./_-]+)/([a-zA-Z0-9\./_-]+)/test(\?.*)?$"
        );
    }


    #[test]
    fn test_route_matches() {
        // Test a request with no captures
        let basic = Route::new(Method::Get, "/url");
        assert_eq!(
            basic.matches(&Method::Get, "/url"),
            Some(vec![])
        );
        assert_eq!(
            basic.matches(&Method::Get, "/url?test"),
            Some(vec!["?test".into()])
        );
        assert_eq!(basic.matches(&Method::Post, "/url"), None);
        assert_eq!(basic.matches(&Method::Get, "/wrong"), None);

        // Test a request with some captures
        let capt = Route::new(Method::Post, "/?/t/?");
        assert_eq!(
            capt.matches(&Method::Post, "/a/t/b"),
            Some(vec!["a".into(), "b".into()])
        );
        assert_eq!(
            capt.matches(&Method::Post, "/a/t/b?hey"),
            Some(vec!["a".into(), "b".into(), "?hey".into()])
        );
        assert_eq!(
            capt.matches(&Method::Post, "/a/t/b.txt"),
            Some(vec!["a".into(), "b.txt".into()])
        );
        assert_eq!(basic.matches(&Method::Post, "/a/t/"), None);
        assert_eq!(basic.matches(&Method::Get, "/a/t/b"), None);
    }


    #[test]
    fn test_handlers() {
        let handler = dummy_handler();

        assert_eq!(
            handler.matches(&Method::Get, "/test"),
            Some(vec!["test".into()])
        );
        assert_eq!(
            handler.call(
                &DummyData(vec!["test".into()]), &dummy_web_request().into(),
                vec!["test".into()]
            ).status(),
            200
        );
    }


    #[test]
    fn test_server() {
        macro_rules! req {
            ($client:expr, $method:expr, $url:expr) => {{
                $client.request($method, &$url).send()
            }};
        }

        // Create the server instance
        let mut server = HttpServer::new(DummyData(vec!["test".into()]), 0);
        server.add_route(Method::Get, "/?", Box::new(dummy_handler_fn));

        // Start the server
        let addr = server.listen("127.0.0.1:0").unwrap();

        let url = format!("http://{}", addr);
        let mut client = hyper::Client::new();

        // Sometimes requests times out after the server was shut down
        // Don't block tests in those cases
        client.set_read_timeout(Some(Duration::new(1, 0)));
        client.set_write_timeout(Some(Duration::new(1, 0)));

        // Make a dummy request
        let res = req!(
            client, hyper::method::Method::Get, format!("{}/test", url)
        ).unwrap();
        assert_eq!(res.status, StatusCode::Ok);

        // Stop the server
        server.stop();

        assert!(
            req!(client, hyper::method::Method::Get, format!("{}/test", url))
            .is_err()
        );
    }
}
