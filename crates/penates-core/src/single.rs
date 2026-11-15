//! Per-app presence on a loopback port derived from the app id.
//! A running app owns the port. A second launch asks it to focus (single-window mode), and the
//! manager/CLI can ask whether an app is running (before clearing its data). The manager uses the
//! same mechanism under its own key (`MANAGER`), so starting Penates twice focuses the open window.
//!
//! Ports are 20000-31999: below the range systems hand out for outgoing connections (32768+ on
//! Linux, 49152+ on Windows and macOS), so a busy port is almost always one of ours. Two ids that
//! hash to the same port lose single-window mode (each message names its id, so they never act on
//! the other's request).

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

/// Key for the manager window. Not a valid app id (ids never contain ':'), so it cannot clash.
pub const MANAGER: &str = "penates:manager";

fn addr(id: &str) -> SocketAddr {
    let mut h: u32 = 2166136261;
    for b in id.bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    SocketAddr::from((Ipv4Addr::LOCALHOST, 20000 + (h % 12000) as u16))
}

/// Become the owner of this app's port. None if another process already holds it.
pub fn try_bind(id: &str) -> Option<TcpListener> {
    TcpListener::bind(addr(id)).ok()
}

fn ask(id: &str, verb: &str) -> bool {
    let Ok(mut s) = TcpStream::connect_timeout(&addr(id), Duration::from_secs(1)) else { return false };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    if s.write_all(format!("penates-{verb}:{id}\n").as_bytes()).is_err() {
        return false;
    }
    let mut buf = [0u8; 2];
    s.read(&mut buf).map(|n| n > 0).unwrap_or(false)
}

/// Ask the running instance to come to the front. True if one answered.
pub fn request_focus(id: &str) -> bool {
    ask(id, "focus")
}

/// Is an instance of this app running right now?
pub fn is_running(id: &str) -> bool {
    ask(id, "ping")
}

/// Answer pings and focus requests; call `on_focus` for the latter.
pub fn serve(listener: TcpListener, id: String, on_focus: impl Fn() + Send + 'static) {
    std::thread::spawn(move || {
        for mut s in listener.incoming().flatten() {
            let _ = s.set_read_timeout(Some(Duration::from_secs(1)));
            let mut buf = [0u8; 256];
            let n = s.read(&mut buf).unwrap_or(0);
            let msg = std::str::from_utf8(&buf[..n]).unwrap_or("").trim();
            if msg == format!("penates-focus:{id}") {
                let _ = s.write_all(b"ok");
                on_focus();
            } else if msg == format!("penates-ping:{id}") {
                let _ = s.write_all(b"ok");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_and_focus_roundtrip() {
        let id = "single-test-app";
        let l = try_bind(id).expect("port free");
        assert!(try_bind(id).is_none()); // second instance cannot bind
        let (tx, rx) = std::sync::mpsc::channel();
        serve(l, id.into(), move || {
            let _ = tx.send(());
        });
        assert!(is_running(id));
        assert!(request_focus(id));
        rx.recv_timeout(Duration::from_secs(2)).expect("focus callback");
        assert!(!is_running("someone-else-entirely"));
    }
}
