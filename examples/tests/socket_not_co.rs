use open_coroutine_examples::{start_client, start_server};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[test]
fn main() -> std::io::Result<()> {
    let addr = "127.0.0.1:9000";
    let server_finished_pair = Arc::new((Mutex::new(true), Condvar::new()));
    let server_finished = Arc::clone(&server_finished_pair);
    _ = std::thread::Builder::new()
        .name("crate_server".to_string())
        .spawn(move || start_server(addr, server_finished_pair))
        .expect("failed to spawn thread");
    _ = std::thread::Builder::new()
        .name("crate_client".to_string())
        .spawn(move || start_client(addr))
        .expect("failed to spawn thread");

    let (lock, cvar) = &*server_finished;
    let result = cvar
        .wait_timeout_while(
            lock.lock().unwrap(),
            Duration::from_secs(30),
            |&mut pending| pending,
        )
        .unwrap();
    if result.1.timed_out() {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "The service did not completed within the specified time",
        ))
    } else {
        Ok(())
    }
}
