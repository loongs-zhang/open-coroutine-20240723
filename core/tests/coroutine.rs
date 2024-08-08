use open_coroutine_core::co;
use open_coroutine_core::common::constants::CoroutineState;
use open_coroutine_core::coroutine::Coroutine;

#[test]
fn coroutine_basic() -> std::io::Result<()> {
    let mut coroutine = co!(|suspender, input| {
        assert_eq!(1, input);
        assert_eq!(3, suspender.suspend_with(2));
        4
    })?;
    assert_eq!(CoroutineState::Suspend(2, 0), coroutine.resume_with(1)?);
    assert_eq!(CoroutineState::Complete(4), coroutine.resume_with(3)?);
    Ok(())
}

#[cfg(not(all(unix, feature = "preemptive")))]
#[test]
fn coroutine_backtrace() -> std::io::Result<()> {
    let mut coroutine = co!(|suspender, input| {
        assert_eq!(1, input);
        println!("{:?}", backtrace::Backtrace::new());
        assert_eq!(3, suspender.suspend_with(2));
        println!("{:?}", backtrace::Backtrace::new());
        4
    })?;
    assert_eq!(CoroutineState::Suspend(2, 0), coroutine.resume_with(1)?);
    assert_eq!(CoroutineState::Complete(4), coroutine.resume_with(3)?);
    Ok(())
}

#[test]
fn coroutine_delay() -> std::io::Result<()> {
    let mut coroutine = co!(|s, ()| {
        let current = Coroutine::<(), (), ()>::current().unwrap();
        assert_eq!(CoroutineState::Running, current.state());
        s.delay(std::time::Duration::MAX);
        unreachable!();
    })?;
    assert_eq!(CoroutineState::Ready, coroutine.state());
    assert_eq!(CoroutineState::Suspend((), u64::MAX), coroutine.resume()?);
    assert_eq!(CoroutineState::Suspend((), u64::MAX), coroutine.state());
    assert_eq!(
        format!(
            "{} unexpected {}->{:?}",
            coroutine.name(),
            CoroutineState::<(), ()>::Suspend((), u64::MAX),
            CoroutineState::<(), ()>::Running
        ),
        coroutine.resume().unwrap_err().to_string()
    );
    assert_eq!(CoroutineState::Suspend((), u64::MAX), coroutine.state());
    Ok(())
}

#[cfg(all(unix, feature = "preemptive"))]
#[test]
fn coroutine_preemptive() -> std::io::Result<()> {
    let pair = std::sync::Arc::new((std::sync::Mutex::new(true), std::sync::Condvar::new()));
    let pair2 = pair.clone();
    _ = std::thread::Builder::new()
        .name("preemptive".to_string())
        .spawn(move || {
            let mut coroutine: Coroutine<(), (), ()> = co!(|_, ()| { loop {} })?;
            assert_eq!(CoroutineState::Suspend((), 0), coroutine.resume()?);
            assert_eq!(CoroutineState::Suspend((), 0), coroutine.state());
            // should execute to here
            let (lock, cvar) = &*pair2;
            let mut pending = lock.lock().unwrap();
            *pending = false;
            cvar.notify_one();
            Ok::<(), std::io::Error>(())
        });
    // wait for the thread to start up
    let (lock, cvar) = &*pair;
    let result = cvar
        .wait_timeout_while(
            lock.lock().unwrap(),
            std::time::Duration::from_millis(1000),
            |&mut pending| pending,
        )
        .unwrap();
    if result.1.timed_out() {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "The monitor should send signals to coroutines in running state",
        ))
    } else {
        Ok(())
    }
}

#[cfg(all(unix, feature = "preemptive"))]
#[test]
fn coroutine_syscall_not_preemptive() -> std::io::Result<()> {
    use open_coroutine_core::common::constants::{Syscall, SyscallState};

    let pair = std::sync::Arc::new((std::sync::Mutex::new(true), std::sync::Condvar::new()));
    let pair2 = pair.clone();
    _ = std::thread::Builder::new()
        .name("syscall_not_preemptive".to_string())
        .spawn(move || {
            let mut coroutine: Coroutine<(), (), ()> = co!(|_, ()| {
                Coroutine::<(), (), ()>::current()
                    .unwrap()
                    .syscall((), Syscall::sleep, SyscallState::Executing)
                    .unwrap();
                loop {}
            })?;
            _ = coroutine.resume()?;
            // should never execute to here
            let (lock, cvar) = &*pair2;
            let mut pending = lock.lock().unwrap();
            *pending = false;
            cvar.notify_one();
            Ok::<(), std::io::Error>(())
        });
    // wait for the thread to start up
    let (lock, cvar) = &*pair;
    let result = cvar
        .wait_timeout_while(
            lock.lock().unwrap(),
            std::time::Duration::from_millis(1000),
            |&mut pending| pending,
        )
        .unwrap();
    if result.1.timed_out() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "The monitor should not send signals to coroutines in syscall state",
        ))
    }
}
