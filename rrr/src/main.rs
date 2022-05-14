use std::net::{SocketAddr, ToSocketAddrs};
use rouille::{start_server, Response};

type Res<A> = Result<A, Box<dyn std::error::Error>>;

fn main() -> Res<()> {
    let result = inner_main();

    if result.is_err() {
        println!("usage: rrr <address>");
    }

    result
}

fn inner_main() -> Res<()> {
    let mut args = std::env::args();

    args.next(); // exe name

    if let Some(addr_str) = args.next() {
        let addr = if let Some(addr) = first_addr(&addr_str) {
            addr
        } else {
            first_addr((addr_str, 8080))
                .ok_or_else(|| "No valid socket address found")?
        };

        start_server(addr, move |request| {
            Response::text(&format!(
                "hello world\n{request:?}"
            ))
        });
    } else {
        Err("No socket address found".into())
    }
}

fn first_addr(to_addrs: impl ToSocketAddrs) -> Option<SocketAddr> {
    to_addrs.to_socket_addrs().ok()?.next()
}