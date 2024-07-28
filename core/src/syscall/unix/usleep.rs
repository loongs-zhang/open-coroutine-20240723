use crate::net::EventLoops;
use crate::syscall::common::reset_errno;
use once_cell::sync::Lazy;
use std::ffi::{c_int, c_uint};
use std::time::Duration;

#[must_use]
pub extern "C" fn usleep(
    fn_ptr: Option<&extern "C" fn(c_uint) -> c_int>,
    microseconds: c_uint,
) -> c_int {
    static CHAIN: Lazy<UsleepSyscallFacade<NioUsleepSyscall>> = Lazy::new(Default::default);
    CHAIN.usleep(fn_ptr, microseconds)
}

trait UsleepSyscall {
    extern "C" fn usleep(
        &self,
        fn_ptr: Option<&extern "C" fn(c_uint) -> c_int>,
        microseconds: c_uint,
    ) -> c_int;
}

impl_facade!(UsleepSyscallFacade, UsleepSyscall,
    usleep(microseconds: c_uint) -> c_int
);

#[derive(Debug, Copy, Clone, Default)]
struct NioUsleepSyscall {}

impl UsleepSyscall for NioUsleepSyscall {
    extern "C" fn usleep(
        &self,
        _: Option<&extern "C" fn(c_uint) -> c_int>,
        microseconds: c_uint,
    ) -> c_int {
        let time = match u64::from(microseconds).checked_mul(1_000) {
            Some(v) => Duration::from_nanos(v),
            None => Duration::MAX,
        };
        _ = EventLoops::wait_event(Some(time));
        reset_errno();
        0
    }
}
