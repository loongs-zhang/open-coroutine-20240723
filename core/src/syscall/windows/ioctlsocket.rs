use crate::syscall::windows::NON_BLOCKING;
use once_cell::sync::Lazy;
use std::ffi::{c_int, c_uint};
use windows_sys::Win32::Networking::WinSock::SOCKET;

#[must_use]
pub extern "system" fn ioctlsocket(
    fn_ptr: Option<&extern "system" fn(SOCKET, c_int, *mut c_uint) -> c_int>,
    fd: SOCKET,
    cmd: c_int,
    argp: *mut c_uint,
) -> c_int {
    static CHAIN: Lazy<IoctlsocketSyscallFacade<NioIoctlsocketSyscall<RawIoctlsocketSyscall>>> =
        Lazy::new(Default::default);
    CHAIN.ioctlsocket(fn_ptr, fd, cmd, argp)
}

trait IoctlsocketSyscall {
    extern "system" fn ioctlsocket(
        &self,
        fn_ptr: Option<&extern "system" fn(SOCKET, c_int, *mut c_uint) -> c_int>,
        fd: SOCKET,
        cmd: c_int,
        argp: *mut c_uint,
    ) -> c_int;
}

impl_facade!(IoctlsocketSyscallFacade, IoctlsocketSyscall,
    ioctlsocket(fd: SOCKET, cmd: c_int, argp: *mut c_uint) -> c_int
);

#[repr(C)]
#[derive(Debug, Default)]
struct NioIoctlsocketSyscall<I: IoctlsocketSyscall> {
    inner: I,
}

impl<I: IoctlsocketSyscall> IoctlsocketSyscall for NioIoctlsocketSyscall<I> {
    extern "system" fn ioctlsocket(
        &self,
        fn_ptr: Option<&extern "system" fn(SOCKET, c_int, *mut c_uint) -> c_int>,
        fd: SOCKET,
        cmd: c_int,
        argp: *mut c_uint,
    ) -> c_int {
        let r = self.inner.ioctlsocket(fn_ptr, fd, cmd, argp);
        if 0 == r {
            if 0 != unsafe { *argp } {
                _ = NON_BLOCKING.insert(fd);
            } else {
                _ = NON_BLOCKING.remove(&fd);
            }
        }
        r
    }
}

impl_raw!(RawIoctlsocketSyscall, IoctlsocketSyscall, windows_sys::Win32::Networking::WinSock,
    ioctlsocket(fd: SOCKET, cmd: c_int, argp: *mut c_uint) -> c_int
);
