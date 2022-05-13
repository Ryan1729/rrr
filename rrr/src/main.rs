use std::net::{SocketAddr, ToSocketAddrs};

type Res<A> = Result<A, Box<dyn std::error::Error>>;

fn main() -> Res<()> {
    let mut args = std::env::args();

    args.next(); // exe name

    if let Some(addr_str) = args.next() {
        let addr = if let Some(addr) = first_addr(&addr_str) {
            addr
        } else {
            first_addr((addr_str, 8080))
                .ok_or_else(|| "No socket address found")?
        };

        println!("{addr:?}");
    } else {
        println!("usage: rrr <address>");
    };

    Ok(())
}

fn first_addr(to_addrs: impl ToSocketAddrs) -> Option<SocketAddr> {
    to_addrs.to_socket_addrs().ok()?.next()
}