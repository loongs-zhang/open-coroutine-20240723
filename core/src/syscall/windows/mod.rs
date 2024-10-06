use dashmap::DashSet;
use once_cell::sync::Lazy;
use windows_sys::Win32::Networking::WinSock::SOCKET;

pub use ioctlsocket::ioctlsocket;
pub use socket::socket;
pub use Sleep::Sleep;
pub use WSASocketW::WSASocketW;

macro_rules! impl_facade {
    ( $struct_name:ident, $trait_name: ident, $syscall: ident($($arg: ident : $arg_type: ty),*) -> $result: ty ) => {
        #[repr(C)]
        #[derive(Debug, Default)]
        struct $struct_name<I: $trait_name> {
            inner: I,
        }

        impl<I: $trait_name> $trait_name for $struct_name<I> {
            extern "system" fn $syscall(
                &self,
                fn_ptr: Option<&extern "system" fn($($arg_type),*) -> $result>,
                $($arg: $arg_type),*
            ) -> $result {
                let syscall = $crate::common::constants::Syscall::$syscall;
                $crate::info!("enter syscall {}", syscall);
                if let Some(co) = $crate::scheduler::SchedulableCoroutine::current() {
                    let new_state = $crate::common::constants::SyscallState::Executing;
                    if co.syscall((), syscall, new_state).is_err() {
                        $crate::error!("{} change to syscall {} {} failed !",
                            co.name(), syscall, new_state);
                    }
                }
                let r = self.inner.$syscall(fn_ptr, $($arg, )*);
                if let Some(co) = $crate::scheduler::SchedulableCoroutine::current() {
                    if co.running().is_err() {
                        $crate::error!("{} change to running state failed !", co.name());
                    }
                }
                $crate::info!("exit syscall {}", syscall);
                r
            }
        }
    }
}

macro_rules! impl_raw {
    ( $struct_name: ident, $trait_name: ident, $($mod_name: ident)::*, $syscall: ident($($arg: ident : $arg_type: ty),*) -> $result: ty ) => {
        #[repr(C)]
        #[derive(Debug, Copy, Clone, Default)]
        struct $struct_name {}

        impl $trait_name for $struct_name {
            extern "system" fn $syscall(
                &self,
                fn_ptr: Option<&extern "system" fn($($arg_type),*) -> $result>,
                $($arg: $arg_type),*
            ) -> $result {
                if let Some(f) = fn_ptr {
                    (f)($($arg),*)
                } else {
                    unsafe { $($mod_name)::*::$syscall($($arg),*) }
                }
            }
        }
    }
}

mod Sleep;
mod WSASocketW;
mod ioctlsocket;
mod socket;

static NON_BLOCKING: Lazy<DashSet<SOCKET>> = Lazy::new(Default::default);

pub extern "C" fn set_errno(errno: windows_sys::Win32::Foundation::WIN32_ERROR) {
    unsafe { windows_sys::Win32::Foundation::SetLastError(errno) }
}

/// # Panics
/// if set fails.
pub extern "C" fn set_non_blocking(fd: SOCKET) {
    assert!(set_non_blocking_flag(fd, true), "set_non_blocking failed !");
}

/// # Panics
/// if set fails.
pub extern "C" fn set_blocking(fd: SOCKET) {
    assert!(set_non_blocking_flag(fd, false), "set_blocking failed !");
}

extern "C" fn set_non_blocking_flag(fd: SOCKET, on: bool) -> bool {
    let non_blocking = is_non_blocking(fd);
    if non_blocking == on {
        return true;
    }
    let mut argp = on.try_into().expect("bool to c_ulong failed !");
    unsafe {
        windows_sys::Win32::Networking::WinSock::ioctlsocket(
            fd,
            windows_sys::Win32::Networking::WinSock::FIONBIO,
            &mut argp,
        ) == 0
    }
}

#[must_use]
pub extern "C" fn is_blocking(fd: SOCKET) -> bool {
    !is_non_blocking(fd)
}

#[must_use]
pub extern "C" fn is_non_blocking(fd: SOCKET) -> bool {
    NON_BLOCKING.contains(&fd)
}
