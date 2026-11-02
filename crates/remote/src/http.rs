//! Just enough HTTP to serve the phone page and hand `/ws` to the
//! WebSocket: the request line and headers, and the checks that keep other
//! web pages out.

use std::io::{self, Read, Write};
use std::net::{IpAddr, TcpStream};
use std::time::{Duration, Instant};

/// Longest request head accepted; a browser's is well under 2 KB.
pub const MAX_HEAD: usize = 8 * 1024;

/// A request's line and headers.
#[derive(Debug, PartialEq)]
pub struct Head {
    pub method: String,
    pub path: String,
    /// Header names in lower case.
    pub headers: Vec<(String, String)>,
    /// Bytes the head takes, up to and including the blank line.
    pub len: usize,
}

impl Head {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Whether this asks to become a WebSocket.
    pub fn is_upgrade(&self) -> bool {
        self.header("upgrade")
            .is_some_and(|u| u.eq_ignore_ascii_case("websocket"))
    }
}

/// Parses a request head from the start of `buf`; `None` until the blank
/// line has arrived, or if it isn't HTTP.
pub fn parse_head(buf: &[u8]) -> Option<Head> {
    let end = buf.windows(4).position(|w| w == b"\r\n\r\n")? + 4;
    let text = std::str::from_utf8(&buf[..end]).ok()?;
    let mut lines = text.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_string();
    let path = first.next()?.to_string();
    if !first.next()?.starts_with("HTTP/1.") {
        return None;
    }
    let headers = lines
        .filter(|l| !l.is_empty())
        .filter_map(|l| {
            let (n, v) = l.split_once(':')?;
            Some((n.trim().to_ascii_lowercase(), v.trim().to_string()))
        })
        .collect();
    Some(Head {
        method,
        path,
        headers,
        len: end,
    })
}

/// Waits for a whole request head without taking it off the socket, so a
/// WebSocket upgrade can still be read by the WebSocket library.
pub fn peek_head(stream: &TcpStream, wait: Duration) -> io::Result<Option<Head>> {
    let deadline = Instant::now() + wait;
    let mut buf = vec![0u8; MAX_HEAD];
    loop {
        let n = match stream.peek(&mut buf) {
            Ok(n) => n,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                0
            }
            Err(e) => return Err(e),
        };
        if let Some(head) = parse_head(&buf[..n]) {
            return Ok(Some(head));
        }
        if n == MAX_HEAD || Instant::now() >= deadline {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Whether the `Host` header names this computer by address, `localhost` or
/// a `.local` name. A web page elsewhere that points its own domain name at
/// this computer (DNS rebinding) sends its domain here, and is refused.
pub fn host_allowed(host: &str) -> bool {
    let name = strip_port(host);
    if name.eq_ignore_ascii_case("localhost") {
        return true;
    }
    if name.parse::<IpAddr>().is_ok() {
        return true;
    }
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".local")
        && lower.len() > ".local".len()
        && lower
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

/// `host[:port]` or `[v6]:port` without the port (and brackets).
fn strip_port(host: &str) -> &str {
    if let Some(rest) = host.strip_prefix('[') {
        return rest.split(']').next().unwrap_or(rest);
    }
    match host.rsplit_once(':') {
        Some((name, port)) if !name.contains(':') && port.chars().all(|c| c.is_ascii_digit()) => {
            name
        }
        _ => host,
    }
}

/// Whether a request may go on: its `Host` names this computer, and a page
/// that sent it (its `Origin`) was served from that same address. Programs
/// that aren't browsers send no `Origin` and still need a device token.
pub fn request_allowed(head: &Head) -> bool {
    let Some(host) = head.header("host") else {
        return false;
    };
    if !host_allowed(host) {
        return false;
    }
    match head.header("origin") {
        None => true,
        Some(origin) => {
            origin.eq_ignore_ascii_case(&format!("http://{host}"))
                || origin.eq_ignore_ascii_case(&format!("https://{host}"))
        }
    }
}

/// Writes a whole response and closes the connection.
pub fn respond(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> io::Result<()> {
    respond_cached(stream, status, content_type, body, "no-store")
}

/// [`respond`] with its own `Cache-Control`, e.g. for album art the phone
/// may keep.
pub fn respond_cached(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
    cache: &str,
) -> io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: {cache}\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Referrer-Policy: no-referrer\r\n\
         X-Frame-Options: DENY\r\n\
         Content-Security-Policy: default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

/// The value of `name` in a path's query string (`/art/x?k=1&s=96`).
pub fn query<'a>(path: &'a str, name: &str) -> Option<&'a str> {
    let (_, q) = path.split_once('?')?;
    q.split('&')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

/// Takes a head already looked at with [`peek_head`] off the socket.
pub fn consume(stream: &mut TcpStream, len: usize) -> io::Result<()> {
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(host: Option<&str>, origin: Option<&str>) -> Head {
        let mut text = String::from("GET /ws HTTP/1.1\r\n");
        if let Some(h) = host {
            text += &format!("Host: {h}\r\n");
        }
        if let Some(o) = origin {
            text += &format!("Origin: {o}\r\n");
        }
        text += "Upgrade: websocket\r\n\r\n";
        parse_head(text.as_bytes()).unwrap()
    }

    #[test]
    fn parses_a_request() {
        let h = parse_head(b"GET / HTTP/1.1\r\nHost: 10.0.0.2:7477\r\nUser-Agent: x\r\n\r\nrest")
            .unwrap();
        assert_eq!(h.method, "GET");
        assert_eq!(h.path, "/");
        assert_eq!(h.header("host"), Some("10.0.0.2:7477"));
        assert_eq!(h.len, 54);
        assert!(!h.is_upgrade());
        assert!(parse_head(b"GET / HTTP/1.1\r\nHost: a").is_none());
        assert!(parse_head(b"hello there\r\n\r\n").is_none());
    }

    #[test]
    fn hosts() {
        assert!(host_allowed("192.168.1.20:7477"));
        assert!(host_allowed("[fe80::1]:7477"));
        assert!(host_allowed("localhost:7477"));
        assert!(host_allowed("Sebas-MacBook.local:7477"));
        assert!(!host_allowed("evil.example.com:7477"));
        assert!(!host_allowed(".local"));
        assert!(!host_allowed("a.local.evil.com"));
    }

    #[test]
    fn origins() {
        assert!(request_allowed(&head(Some("192.168.1.20:7477"), None)));
        assert!(request_allowed(&head(
            Some("192.168.1.20:7477"),
            Some("http://192.168.1.20:7477")
        )));
        // Another web page, or a rebound domain name.
        assert!(!request_allowed(&head(
            Some("192.168.1.20:7477"),
            Some("http://evil.example.com")
        )));
        assert!(!request_allowed(&head(
            Some("evil.example.com:7477"),
            Some("http://evil.example.com:7477")
        )));
        assert!(!request_allowed(&head(None, None)));
    }
}
