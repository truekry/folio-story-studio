//! Tests for the one-instance guard and its wake-up socket.

use super::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

static NEXT: AtomicU64 = AtomicU64::new(1);

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "folio-instance-{}-{}-{}",
        std::process::id(),
        name,
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temporary data folder");
    dir
}

#[test]
fn a_second_start_does_not_get_the_folder() {
    let dir = temp_dir("second");
    let first = start(&dir);
    assert!(
        matches!(&first, Start::First(_)),
        "the first start owns the folder"
    );
    assert!(
        matches!(start(&dir), Start::Running),
        "the second start must not become a second writer"
    );
    // The guard is still alive here: dropping it releases the folder.
    drop(first);
    assert!(
        matches!(start(&dir), Start::First(_)),
        "after the first window closed the folder is free again"
    );
}

#[test]
fn the_socket_wakes_the_running_instance() {
    let dir = temp_dir("wake");
    let flag = Arc::new(AtomicBool::new(false));
    let woken = Arc::clone(&flag);
    serve(&dir, move || woken.store(true, Ordering::Relaxed)).expect("the wake-up socket");

    // The port is written before the thread starts, so a start finds it.
    assert!(matches!(start(&dir), Start::Running));
    for _ in 0..50 {
        if flag.load(Ordering::Relaxed) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        flag.load(Ordering::Relaxed),
        "the running window must be asked to come forward"
    );
}

#[test]
fn a_stale_port_file_does_not_stop_the_start() {
    let dir = temp_dir("stale");
    // A port nobody listens on, as a crashed instance would leave behind.
    let listener =
        std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).expect("a free port");
    let port = listener.local_addr().expect("the port").port();
    drop(listener);
    fs::write(dir.join("folio.port"), port.to_string()).expect("a stale port");
    assert!(matches!(start(&dir), Start::First(_)));
}

#[test]
fn a_foreign_service_on_the_port_is_not_folio() {
    let dir = temp_dir("foreign");
    // Something else answers, but never with Folio's token.
    let listener =
        std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).expect("a free port");
    let port = listener.local_addr().expect("the port").port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            drop(stream);
        }
    });
    fs::write(dir.join("folio.port"), port.to_string()).expect("the port file");
    assert!(matches!(start(&dir), Start::First(_)));
}

#[test]
fn two_data_folders_run_side_by_side() {
    let a = temp_dir("folders-a");
    let b = temp_dir("folders-b");
    serve(&a, || {}).expect("serve the first folder");
    serve(&b, || {}).expect("serve the second folder");
    assert!(matches!(start(&a), Start::Running));
    assert!(matches!(start(&b), Start::Running));

    // A folder nobody uses is still free.
    let c = temp_dir("folders-c");
    assert!(matches!(start(&c), Start::First(_)));
}
