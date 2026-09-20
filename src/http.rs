//! The loopback HTTP client (the family pattern: hand-rolled, http://
//! only, short timeouts). The pilot's client hardcodes 127.0.0.1; the
//! kit speaks to its own registry peer at the address the operator
//! configured (`KIT_BIND`:`KIT_PORT`), so the host is a parameter here,
//! and the admin mutations carry a Bearer token.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

pub struct HttpText {
    pub status: u16,
    pub body: String,
}

pub fn request(
    method: &str,
    host: &str,
    port: u16,
    path: &str,
    body: Option<&str>,
    bearer: Option<&str>,
    timeout: Duration,
) -> Option<HttpText> {
    let mut stream = TcpStream::connect((host, port)).ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;
    let mut head =
        format!("{method} {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n");
    if let Some(token) = bearer {
        head.push_str(&format!("authorization: Bearer {token}\r\n"));
    }
    if body.is_some() {
        head.push_str("content-type: application/json\r\n");
    }
    head.push_str(&format!(
        "content-length: {}\r\n\r\n",
        body.map(str::len).unwrap_or(0)
    ));
    stream.write_all(head.as_bytes()).ok()?;
    if let Some(body) = body {
        stream.write_all(body.as_bytes()).ok()?;
    }
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).ok()?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let status = text.split_whitespace().nth(1)?.parse().ok()?;
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    Some(HttpText { status, body })
}

pub fn get(host: &str, port: u16, path: &str, timeout: Duration) -> Option<HttpText> {
    request("GET", host, port, path, None, None, timeout)
}

pub fn get_auth(
    host: &str,
    port: u16,
    path: &str,
    bearer: &str,
    timeout: Duration,
) -> Option<HttpText> {
    request("GET", host, port, path, None, Some(bearer), timeout)
}

pub fn post_auth(
    host: &str,
    port: u16,
    path: &str,
    body: &str,
    bearer: &str,
    timeout: Duration,
) -> Option<HttpText> {
    request("POST", host, port, path, Some(body), Some(bearer), timeout)
}
