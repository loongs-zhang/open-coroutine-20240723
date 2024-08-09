use crate::net::EventLoops;
use once_cell::sync::Lazy;
use std::time::Duration;

pub fn Sleep(fn_ptr: Option<fn(u32)>, dw_milliseconds: u32) {
    static CHAIN: Lazy<SleepSyscallFacade<NioSleepSyscall>> = Lazy::new(Default::default);
    CHAIN.Sleep(fn_ptr, dw_milliseconds);
}

trait SleepSyscall {
    fn Sleep(&self, fn_ptr: Option<fn(u32)>, dw_milliseconds: u32);
}

impl_facade!(SleepSyscallFacade, SleepSyscall, Sleep(dw_milliseconds: u32) -> ());

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
struct NioSleepSyscall {}

impl SleepSyscall for NioSleepSyscall {
    fn Sleep(&self, _: Option<fn(u32)>, dw_milliseconds: u32) {
        let time = Duration::from_millis(u64::from(dw_milliseconds));
        _ = EventLoops::wait_event(Some(time));
    }
}
