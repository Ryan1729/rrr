use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use rouille::{start_server, try_or_400, Request, Response};

use logic::Method;

type Res<A> = Result<A, Box<dyn std::error::Error>>;

const APP_NAME: &str = env!("CARGO_PKG_NAME");

fn main() -> Res<()> {
    let result = inner_main();

    if result.is_err() {
        println!("usage: {APP_NAME} <address>");
    }

    result
}

fn inner_main() -> Res<()> {
    let mut args = std::env::args();

    args.next(); // exe name

    let addr = if let Some(addr_str) = args.next() {
        if let Some(addr) = first_addr(&addr_str) {
            addr
        } else {
            first_addr((addr_str, 8080))
                .ok_or_else(|| "No valid socket address found")?
        }
    } else {
        return Err("No socket address found".into())
    };

    println!("Address: {addr}");

    let data_dir = if let Some(data_dir_override) = args.next()
        // Treat an empty arg as no arg.
        .and_then(|s| (!s.is_empty()).then(|| s))
    {
        PathBuf::try_from(data_dir_override)?
    } else {
        directories::ProjectDirs::from("com", "ryanwiedemann", APP_NAME)
            // The `directories` docs says this only returns none when
            // "no valid home directory path could be retrieved from
            // the operating system."
            .ok_or_else(|| "No valid home directory path found")?
            .data_dir()
            .to_owned()
    };

    let state = logic::State::try_from(data_dir)?;

    {
        let displayed_dir = state.root_display();
        println!("Data Directory: {displayed_dir}");
    }

    start(addr, state)
}

fn first_addr(to_addrs: impl ToSocketAddrs) -> Option<SocketAddr> {
    to_addrs.to_socket_addrs().ok()?.next()
}

#[repr(transparent)]
struct TaskSpec<'request>(&'request Request);

impl <'request> logic::TaskSpec for TaskSpec<'request> {
    fn method(&self) -> Method {
        match self.0.method() {
            "GET" => Method::Get,
            "POST" => Method::Post,
            _ => Method::Other,
        }
    }

    fn url_suffix(&self) -> String {
        self.0.url()
    }

    fn query_param(&self, key: &str) -> Option<String> {
        self.0.get_param(key)
    }

    type LocalAddFormError = rouille::input::post::PostError;
    fn local_add_form(&self)
    -> Result<Vec<(String, String)>, Self::LocalAddFormError> {
        rouille::input::post::raw_urlencoded_post_input(
            &self.0
        )
    }

    type RemoteFeedAddFormError = rouille::input::post::PostError;
    fn remote_feed_add_form(&self)
    -> Result<Vec<(String, String)>, Self::RemoteFeedAddFormError> {
        rouille::input::post::raw_urlencoded_post_input(
            &self.0
        )
    }
}

fn start(addr: SocketAddr, state: logic::State) -> ! {
    let state_mutex = std::sync::Mutex::new(state);

    start_server(addr, move |request| {
        match state_mutex.lock() {
            Ok(ref mut state) => {
                let task: logic::Task = try_or_400!(
                    logic::extract_task(&TaskSpec(&request), state)
                );

                match state.perform(task) {
                    Ok(output) => extract_response(output),
                    Err(e) => {
                        Response::text(e.to_string()).with_status_code(500)
                    }
                }
            }
            Err(e) => {
                Response::text(e.to_string()).with_status_code(503)
            }
        }
    })
}

fn extract_response(output: logic::Output) -> Response {
    use logic::Output::*;

    match output {
        Html(html) => Response::html(html),
    }
}