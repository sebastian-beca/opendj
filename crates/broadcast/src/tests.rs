use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;

/// Hands out a 440 Hz tone, as much as real time asks for.
struct Tone {
    rate: u32,
    at: u64,
    since: Instant,
    active: Arc<AtomicBool>,
}

impl Source for Tone {
    fn drain_into(&mut self, out: &mut Vec<f32>) {
        if !self.active.load(Ordering::Acquire) {
            return;
        }
        let due = (self.since.elapsed().as_secs_f64() * self.rate as f64) as u64;
        while self.at < due {
            let v = (0.3
                * (std::f64::consts::TAU * 440.0 * self.at as f64 / self.rate as f64).sin())
                as f32;
            out.extend([v, v]);
            self.at += 1;
        }
    }

    fn set_active(&self, on: bool) {
        self.active.store(on, Ordering::Release);
    }
}

fn tone(rate: u32) -> (Box<dyn Source>, Arc<AtomicBool>) {
    let active = Arc::new(AtomicBool::new(false));
    (
        Box::new(Tone {
            rate,
            at: 0,
            since: Instant::now(),
            active: active.clone(),
        }),
        active,
    )
}

/// Reads one source connection: answers the PUT, then collects the stream
/// for `seconds` (or until it closes). Returns the request and the bytes.
fn accept_source(l: &TcpListener, seconds: f64) -> (String, Vec<u8>) {
    let (mut s, _) = l.accept().unwrap();
    s.set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    let mut got = Vec::new();
    let mut buf = [0u8; 8192];
    let head_end = loop {
        let n = s.read(&mut buf).unwrap_or(0);
        got.extend_from_slice(&buf[..n]);
        if let Some(i) = got.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    s.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").unwrap();
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < seconds {
        match s.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => got.extend_from_slice(&buf[..n]),
            Err(_) => {}
        }
    }
    let body = got.split_off(head_end);
    (String::from_utf8_lossy(&got).into_owned(), body)
}

fn station(port: u16, format: Format) -> Target {
    Target {
        station: Station {
            id: "test".into(),
            name: "Test".into(),
            host: "127.0.0.1".into(),
            port,
            format,
            ..Station::default()
        },
        password: "hackme".into(),
    }
}

fn wait_for(what: impl Fn() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if what() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn goes_on_air_streams_opus_with_the_title_and_comes_off() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let reg = ControlRegistry::new();
    let (source, active) = tone(44_100);
    let now: NowPlaying = Arc::new(|| Some(("Moby".to_string(), "Porcelain".to_string())));
    let b = Broadcaster::new(&reg, 44_100, source, now);
    b.configure(Settings {
        targets: vec![station(port, Format::Opus)],
        title_template: String::new(),
        dj_name: String::new(),
    });
    // ON AIR from a controller.
    reg.get("[Shoutcast]", "enabled").unwrap().set(1.0);
    let (req, body) = accept_source(&l, 1.5);
    assert!(req.starts_with("PUT /live HTTP/1.1"), "{req}");
    assert!(active.load(Ordering::Acquire));
    assert!(
        body.starts_with(b"OggS"),
        "{:?}",
        &body[..body.len().min(8)]
    );
    // The first title goes out at once, in the stream headers.
    assert!(body.windows(15).any(|w| w == b"TITLE=Porcelain"));
    assert!(wait_for(
        || reg.get("[Shoutcast]", "status").unwrap().get() == 2.0
    ));
    assert_eq!(b.statuses()[0].state, State::Connected);
    reg.get("[Shoutcast]", "enabled").unwrap().set(0.0);
    assert!(wait_for(
        || reg.get("[Shoutcast]", "status").unwrap().get() == 0.0
    ));
    assert!(!b.is_on_air());
    assert!(wait_for(|| !active.load(Ordering::Acquire)));
}

#[test]
fn reconnects_when_the_server_drops_the_stream() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let reg = ControlRegistry::new();
    let (source, _) = tone(48_000);
    let b = Broadcaster::new(&reg, 48_000, source, Arc::new(|| None));
    b.configure(Settings {
        targets: vec![station(port, Format::Mp3)],
        ..Settings::default()
    });
    b.start().unwrap();
    // The server hangs up after half a second...
    let (_, first) = accept_source(&l, 0.5);
    assert!(!first.is_empty());
    // ...and the station comes straight back.
    let (req, again) = accept_source(&l, 0.5);
    assert!(req.contains("Content-Type: audio/mpeg"));
    assert!(!again.is_empty());
    b.stop();
}

#[test]
fn a_wrong_password_fails_without_retrying() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = s.read(&mut buf);
        s.write_all(b"HTTP/1.1 401 Authentication Required\r\n\r\n")
            .unwrap();
        // No second try comes.
        l.set_nonblocking(true).unwrap();
        std::thread::sleep(Duration::from_millis(500));
        l.accept().is_err()
    });
    let reg = ControlRegistry::new();
    let (source, _) = tone(48_000);
    let b = Broadcaster::new(&reg, 48_000, source, Arc::new(|| None));
    b.configure(Settings {
        targets: vec![station(port, Format::Opus)],
        ..Settings::default()
    });
    b.start().unwrap();
    assert!(wait_for(|| b.statuses()[0].state == State::Failed));
    assert_eq!(
        b.statuses()[0].message,
        "The server turned down the user name or password"
    );
    assert!(wait_for(
        || reg.get("[Shoutcast]", "status").unwrap().get() == 3.0
    ));
    assert!(server.join().unwrap());
    b.stop();
}

#[test]
fn nothing_to_stream_to_is_an_error() {
    let reg = ControlRegistry::new();
    let (source, _) = tone(48_000);
    let b = Broadcaster::new(&reg, 48_000, source, Arc::new(|| None));
    assert!(b.start().is_err());
    assert!(!b.is_on_air());
}

#[test]
fn title_text_fills_in_the_template() {
    assert_eq!(title_text("", "", "Moby", "Porcelain"), "Moby - Porcelain");
    assert_eq!(
        title_text("{title} ({artist})", "", "Moby", "Porcelain"),
        "Porcelain (Moby)"
    );
    assert_eq!(
        title_text("Live with {dj}", "DJ Example", "Moby", "Porcelain"),
        "Live with DJ Example"
    );
    assert_eq!(title_text("", "", "", "Porcelain"), "Porcelain");
}

#[test]
fn station_status_summary() {
    let s = |state| StationStatus {
        state,
        ..Default::default()
    };
    assert_eq!(summary(&[]), 0.0);
    assert_eq!(summary(&[s(State::Connected), s(State::Connected)]), 2.0);
    assert_eq!(summary(&[s(State::Connected), s(State::Connecting)]), 4.0);
    assert_eq!(summary(&[s(State::Failed), s(State::Failed)]), 3.0);
    assert_eq!(summary(&[s(State::Connecting), s(State::Failed)]), 1.0);
}
