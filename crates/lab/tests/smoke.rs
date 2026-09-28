//! Real-socket smoke test: start the server on an OS-assigned free port, fetch a couple of
//! endpoints over a raw HTTP/1.1 `TcpStream`, then shut it down.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dominion_lab::AppState;

fn strategies_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../strategies"))
}

fn get(port: u16, path: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    write!(stream, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").unwrap();
    stream.flush().unwrap();
    let mut resp = String::new();
    stream.read_to_string(&mut resp).expect("read response");
    let status_line = resp.lines().next().expect("status line");
    let status: u16 = status_line.split_whitespace().nth(1).expect("status code").parse().expect("parse status");
    (status, resp)
}

#[test]
fn serves_over_a_real_socket() {
    let state = AppState::new(strategies_dir());
    let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").expect("bind to an OS-assigned port"));
    let port = match server.server_addr() {
        tiny_http::ListenAddr::IP(addr) => addr.port(),
        #[allow(unreachable_patterns)]
        _ => panic!("expected an IP listen address"),
    };

    let loop_server = server.clone();
    let handle = std::thread::spawn(move || {
        dominion_lab::accept_loop(state, loop_server);
    });

    let (status, body) = get(port, "/");
    assert_eq!(status, 200);
    assert!(body.contains("Dominion Strategy Lab"), "page body: {body}");

    let (status, body) = get(port, "/api/cards");
    assert_eq!(status, 200);
    assert!(body.contains("\"kingdom\""), "cards body: {body}");

    server.unblock();
    handle.join().expect("server thread panicked");
}
