use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use rouille::{start_server, Response};

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

    println!("Data Directory: {}", data_dir.display());

    start(addr, data_dir)
}

fn first_addr(to_addrs: impl ToSocketAddrs) -> Option<SocketAddr> {
    to_addrs.to_socket_addrs().ok()?.next()
}

fn start(addr: SocketAddr, data_dir: PathBuf) -> ! {
    start_server(addr, move |request| {
        Response::text(&format!(
            "hello world\n{request:?}\n{data_dir:?}"
        ))
    })
}