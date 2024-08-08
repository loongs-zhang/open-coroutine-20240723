use open_coroutine_core::co_pool::CoroutinePool;
use open_coroutine_core::scheduler::SchedulableSuspender;
use std::time::Duration;

#[test]
fn co_pool_basic() -> std::io::Result<()> {
    let task_name = "test_simple";
    let mut pool = CoroutinePool::default();
    pool.set_max_size(1);
    assert!(pool.is_empty());
    _ = pool.submit_task(
        Some(String::from("test_panic")),
        |_| panic!("test panic, just ignore it"),
        None,
    )?;
    assert!(!pool.is_empty());
    pool.submit_task(
        Some(String::from(task_name)),
        |_| {
            println!("2");
            Some(2)
        },
        None,
    )?;
    pool.try_schedule_task()
}

#[test]
fn co_pool_suspend() -> std::io::Result<()> {
    let mut pool = CoroutinePool::default();
    pool.set_max_size(2);
    _ = pool.submit_task(
        None,
        |param| {
            println!("[coroutine] delay");
            if let Some(suspender) = SchedulableSuspender::current() {
                suspender.delay(Duration::from_millis(100));
            }
            println!("[coroutine] back");
            param
        },
        None,
    )?;
    _ = pool.submit_task(
        None,
        |_| {
            println!("middle");
            Some(1)
        },
        None,
    )?;
    pool.try_schedule_task()?;
    std::thread::sleep(Duration::from_millis(200));
    pool.try_schedule_task()
}

#[test]
fn co_pool_stop() -> std::io::Result<()> {
    let pool = CoroutinePool::default();
    pool.set_max_size(1);
    _ = pool.submit_task(None, |_| panic!("test panic, just ignore it"), None)?;
    pool.submit_task(
        None,
        |_| {
            println!("2");
            Some(2)
        },
        None,
    )
}
