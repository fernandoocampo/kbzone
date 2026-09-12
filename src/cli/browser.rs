//! One-shot local HTTP hand-off for `kb graph`: serves a single in-memory
//! HTML response over a loopback TCP connection, opens the platform's
//! default browser at that URL, then returns. No file is ever written to
//! disk — the whole page is self-contained, so the browser only ever needs
//! that one request.
//!
//! Plain OS threads + blocking `std::net`/`std::sync::mpsc` — no async
//! runtime, consistent with the rest of this project.

use std::io::Write;
use std::net::TcpListener;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::errors::Error;

/// How long to wait for the browser to connect before giving up and
/// printing the URL for the user to open manually.
const ACCEPT_TIMEOUT: Duration = Duration::from_secs(10);

/// Serves `html` once over `http://127.0.0.1:<port>` and opens it in the
/// user's default browser. Blocks until that request is served or
/// `ACCEPT_TIMEOUT` elapses, whichever comes first.
pub fn serve_once_and_open(html: &str) -> Result<(), Error> {
    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|e| Error::GraphQueryError(e.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|e| Error::GraphQueryError(e.to_string()))?
        .port();
    let url = format!("http://127.0.0.1:{port}");
    println!("Serving graph at {url}");

    let (tx, rx) = mpsc::channel();
    let html = html.to_string();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                html.len(),
                html
            );
            let _ = stream.write_all(response.as_bytes());
        }
        let _ = tx.send(());
    });

    if open_url(&url).is_err() {
        println!("Could not launch a browser automatically — open {url} manually.");
    }

    if rx.recv_timeout(ACCEPT_TIMEOUT).is_err() {
        println!("Timed out waiting for the browser — open {url} manually.");
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn open_url(url: &str) -> Result<(), Error> {
    run_open_command(Command::new("open").arg(url))
}

#[cfg(target_os = "linux")]
fn open_url(url: &str) -> Result<(), Error> {
    run_open_command(Command::new("xdg-open").arg(url))
}

#[cfg(target_os = "windows")]
fn open_url(url: &str) -> Result<(), Error> {
    run_open_command(Command::new("cmd").args(["/c", "start", "", url]))
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn run_open_command(command: &mut Command) -> Result<(), Error> {
    let status = command
        .status()
        .map_err(|e| Error::GraphQueryError(e.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::GraphQueryError(
            "browser launch command failed".to_string(),
        ))
    }
}
