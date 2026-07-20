use flume::Sender;
use std::io::Write;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub fn spawn(addr: &str) -> ((), Sender<Vec<u8>>) {
    let listener = TcpListener::bind(addr).expect("[HAYAKU] Bind failed");
    eprintln!("[HAYAKU] Listening on {}", addr);

    let clients = Arc::new(Mutex::new(Vec::with_capacity(8)));
    let clients_broad = clients.clone();

    // Accept loop
    thread::spawn(move || {
        loop {
            match listener.accept() {
                Ok((stream, _addr)) => {
                    if let Err(e) = stream.set_write_timeout(Some(Duration::from_millis(50))) {
                        eprintln!("[HAYAKU] Set write timeout err: {e}");
                    }
                    clients_broad.lock().unwrap().push(stream.try_clone().expect("Clone failed"));
                }
                Err(e) => {
                    if e.kind() != std::io::ErrorKind::WouldBlock {
                        eprintln!("[HAYAKU] Accept err: {e}");
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }
    });

    let (tx, rx) = flume::bounded::<Vec<u8>>(2048);
    let clients_bcast = clients.clone();

    // Broadcast loop
    thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
            let mut guards = clients_bcast.lock().unwrap();
            let mut idx = 0;
            while idx < guards.len() {
                match (guards[idx].write_all(&msg), guards[idx].flush()) {
                    (Ok(()), Ok(())) => idx += 1,
                    _ => {
                        eprintln!("[HAYAKU] Evicting dead client");
                        guards.swap_remove(idx);
                    }
                }
            }
        }
    });

    ((), tx)
}
