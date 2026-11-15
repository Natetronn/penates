//! Per-app single instance via a loopback port derived from the app id.
//! A second launch connects, asks the first to focus, and exits.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::time::Duration;

pub enum Claim {
    Primary(TcpListener),
    Secondary,
    /// Port taken by something else; just run without single-instance.
    Unavailable,
}

fn port(id: &str) -> u16 {
    let mut h: u32 = 2166136261;
    for b in id.bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    49152 + (h % 16000) as u16
}

pub fn claim(id: &str) -> Claim {
    let addr = (Ipv4Addr::LOCALHOST, port(id));
    if let Ok(l) = TcpListener::bind(addr) {
        return Claim::Primary(l);
    }
    if let Ok(mut s) = TcpStream::connect_timeout(&addr.into(), Duration::from_secs(1)) {
        let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
        if s.write_all(format!("penates-focus:{id}\n").as_bytes()).is_ok() {
            let mut buf = [0u8; 2];
            if s.read(&mut buf).map(|n| n > 0).unwrap_or(false) {
                return Claim::Secondary;
            }
        }
    }
    Claim::Unavailable
}

/// Call `on_focus` whenever another launch asks us to come forward.
pub fn serve(listener: TcpListener, id: String, on_focus: impl Fn() + Send + 'static) {
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut s = stream;
            let _ = s.set_read_timeout(Some(Duration::from_secs(1)));
            let mut buf = [0u8; 256];
            let n = s.read(&mut buf).unwrap_or(0);
            if std::str::from_utf8(&buf[..n]).is_ok_and(|t| t.trim() == format!("penates-focus:{id}")) {
                let _ = s.write_all(b"ok");
                on_focus();
            }
        }
    });
}
