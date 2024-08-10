use open_coroutine::task;
use open_coroutine_core::common::now;
use std::time::Duration;

fn sleep_test(millis: u64) {
    _ = task!(
        move |_| {
            println!("[coroutine1] {millis} launched");
        },
        (),
    );
    _ = task!(
        move |_| {
            println!("[coroutine2] {millis} launched");
        },
        (),
    );
    let start = now();
    std::thread::sleep(Duration::from_millis(millis));
    let end = now();
    assert!(end - start >= millis, "Time consumption less than expected");
}

#[test]
#[open_coroutine::main(event_loop_size = 1, max_size = 2)]
fn main() {
    sleep_test(1);
    sleep_test(1000);
}
