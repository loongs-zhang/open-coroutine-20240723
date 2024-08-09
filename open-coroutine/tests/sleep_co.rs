use open_coroutine::task;
use open_coroutine_core::common::now;
use std::time::Duration;

pub fn sleep_test_co(millis: u64) {
    _ = task!(
        move |_| {
            let start = now();
            std::thread::sleep(Duration::from_millis(millis));
            let end = now();
            assert!(end - start >= millis, "Time consumption less than expected");
            println!("[coroutine1] {millis} launched");
        },
        (),
    );
    _ = task!(
        move |_| {
            std::thread::sleep(Duration::from_millis(500));
            println!("[coroutine2] {millis} launched");
        },
        (),
    );
    std::thread::sleep(Duration::from_millis(millis + 500));
}

#[test]
#[open_coroutine::main(event_loop_size = 1, max_size = 2, keep_alive_time = 0)]
fn main() {
    sleep_test_co(1);
    sleep_test_co(1000);
}
