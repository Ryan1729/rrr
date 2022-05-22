use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use rouille::{start_server, try_or_400, Request, Response};

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

    let data_dir = if let Some(data_dir_override) = args.next() {
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
    std::fs::create_dir_all(&data_dir)?;
    if !data_dir.is_dir() {
        return Err(format!("Not a directory: {}", data_dir.display()).into())
    }
    let data_dir = data_dir.canonicalize()?;

    let displayed_dir = data_dir.display().to_string();

    let state = logic::State::try_from(data_dir)?;

    println!("Data Directory: {displayed_dir}");

    start(addr, state)
}

fn first_addr(to_addrs: impl ToSocketAddrs) -> Option<SocketAddr> {
    to_addrs.to_socket_addrs().ok()?.next()
}

fn start(addr: SocketAddr, state: logic::State) -> ! {
    let state_mutex = std::sync::Mutex::new(state);

    start_server(addr, move |request| {
        let task: logic::Task = try_or_400!(extract_task(&request));

        match state_mutex.lock() {
            Ok(ref mut state) => {
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

/// This exisits because if we try to use `Box<dyn std::error::Error>` we get
/// "the size for values of type `dyn std::error::Error` cannot be known at
/// compilation time" from `try_or_400`.
#[derive(Debug)]
struct TaskError(String);

impl core::fmt::Display for TaskError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for TaskError {}

fn extract_task(request: &Request) -> Result<logic::Task, TaskError> {
    use logic::Task::*;

    let url = request.url();
    match (request.method(), url.as_str()) {
        ("GET", "/") => {
            Ok(ShowHomePage)
        },
        (method, _) => {
            Err(TaskError(
                format!(
                    "No known task for HTTP {method} method at url {url}"
                )
            ))
        },
    }
}

fn extract_response(output: logic::Output) -> Response {
    use logic::Output::*;

    match output {
        Html(html) => Response::html(html),
    }
}