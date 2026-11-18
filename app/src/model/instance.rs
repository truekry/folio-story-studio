//! One Folio per data folder.
//!
//! Two windows writing into the same `stories\` folder would overwrite each
//! other. A lock file next to `config.json` prevents that, and a loopback
//! socket lets a second start bring the running window back instead of opening
//! another one.
//!
//! Nothing crosses the socket except "activate" and a process id; neither the
//! books nor any setting travel through it.

use super::*;
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

const LOCK_FILE: &str = "folio.lock";
const PORT_FILE: &str = "folio.port";
const HELLO: &str = "FOLIO-ACTIVATE";

/// The lock this process holds for as long as it runs. Dropping it (or the
/// process ending) releases the folder.
pub(crate) struct Guard {
    _file: fs::File,
}

/// What a start found.
pub(crate) enum Start {
    /// This process owns the data folder and should call [`serve`].
    First(Guard),
    /// Another Folio is running; it was asked to come forward when possible.
    Running,
    /// The folder cannot be locked (read-only, missing). Folio runs anyway.
    Unguarded(String),
}

/// Decide whether this process may open the data folder. This runs before the
/// window is created, so a second start never flashes a window of its own.
pub(crate) fn start(dir: &Path) -> Start {
    // The fast path: a running instance answers on the port it published.
    if let Some(port) = read_port(dir) {
        if activate(port, &token(dir)) {
            return Start::Running;
        }
    }
    match lock(dir) {
        Ok(Some(guard)) => Start::First(guard),
        Ok(None) => {
            // The folder is locked but the socket did not answer: the other
            // instance is still starting up, or its port file was lost. Give it
            // a moment, then stay closed rather than become a second writer.
            for _ in 0..5 {
                std::thread::sleep(Duration::from_millis(100));
                if let Some(port) = read_port(dir) {
                    if activate(port, &token(dir)) {
                        break;
                    }
                }
            }
            Start::Running
        }
        Err(error) => Start::Unguarded(error),
    }
}

/// Bind a loopback listener, publish its port and hand every activation to
/// `wake`. The port is chosen by the OS, so two data folders never collide.
pub(crate) fn serve(dir: &Path, wake: impl Fn() + Send + 'static) -> Result<(), String> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|error| format!("no loopback socket: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("no loopback port: {error}"))?
        .port();
    write_port(dir, port)?;
    let token = token(dir);
    std::thread::Builder::new()
        .name("folio-instance".to_string())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                if handshake(&stream, &token) {
                    wake();
                }
            }
        })
        .map_err(|error| format!("no instance thread: {error}"))?;
    Ok(())
}

/// A stable fingerprint of the data folder. It keeps a stray local service on
/// a reused port from being mistaken for Folio.
fn token(dir: &Path) -> String {
    let path = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let text = path.to_string_lossy();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn port_path(dir: &Path) -> PathBuf {
    dir.join(PORT_FILE)
}

fn read_port(dir: &Path) -> Option<u16> {
    let text = fs::read_to_string(port_path(dir)).ok()?;
    text.trim().parse::<u16>().ok().filter(|port| *port != 0)
}

fn write_port(dir: &Path, port: u16) -> Result<(), String> {
    let path = port_path(dir);
    fs::write(&path, port.to_string())
        .map_err(|error| format!("{} could not be written: {error}", path.display()))
}

/// Ask the running instance to come forward. True when it answered.
fn activate(port: u16, token: &str) -> bool {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(300)) else {
        return false;
    };
    let _ = stream.set_write_timeout(Some(Duration::from_millis(300)));
    let _ = stream.set_read_timeout(Some(Duration::from_millis(300)));
    if writeln!(stream, "{HELLO} {token}").is_err() {
        return false;
    }
    let mut reply = String::new();
    if BufReader::new(&stream).read_line(&mut reply).is_err() {
        return false;
    }
    let Some(pid) = reply.trim().strip_prefix("OK ") else {
        return false;
    };
    if let Ok(pid) = pid.trim().parse::<u32>() {
        // The second start still owns the foreground right; hand it to the
        // running window, or Windows will only blink it in the taskbar.
        allow_foreground(pid);
    }
    true
}

/// Read one request and answer with this process's id when the token matches.
fn handshake(mut stream: &TcpStream, token: &str) -> bool {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    let mut line = String::new();
    if BufReader::new(stream).read_line(&mut line).is_err() {
        return false;
    }
    if line.trim() != format!("{HELLO} {token}") {
        return false;
    }
    let _ = writeln!(stream, "OK {}", std::process::id());
    true
}

fn lock(dir: &Path) -> Result<Option<Guard>, String> {
    let path = dir.join(LOCK_FILE);
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(Guard { _file: file })),
        Err(fs::TryLockError::WouldBlock) => Ok(None),
        Err(fs::TryLockError::Error(error)) => {
            Err(format!("{} could not be locked: {error}", path.display()))
        }
    }
}

#[cfg(windows)]
fn allow_foreground(pid: u32) {
    // SAFETY: no pointers, no memory is read or written. The call only grants
    // one process the right to take the foreground.
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(pid);
    }
}

#[cfg(not(windows))]
fn allow_foreground(_pid: u32) {
    // X11 and the Mac window manager decide on their own; the focus request
    // from the running window is all Folio can do there.
}
