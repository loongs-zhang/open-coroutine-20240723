use crate::impl_display_by_debug;
use once_cell::sync::Lazy;

/// Recommended stack size for coroutines.
pub const DEFAULT_STACK_SIZE: usize = 128 * 1024;

/// Get the cpu count
#[must_use]
pub fn cpu_count() -> usize {
    static CPU_COUNT: Lazy<usize> = Lazy::new(num_cpus::get);
    *CPU_COUNT
}

/// Enums used to describe pool state
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum PoolState {
    /// The pool is running.
    Running,
    /// The pool is stopping.
    Stopping,
    /// The pool is stopped.
    Stopped,
}

impl_display_by_debug!(PoolState);

/// Enums used to describe syscall
#[allow(non_camel_case_types, missing_docs)]
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Syscall {
    #[cfg(windows)]
    Sleep,
    sleep,
    usleep,
    nanosleep,
    poll,
    select,
    #[cfg(target_os = "linux")]
    accept4,
    #[cfg(target_os = "linux")]
    epoll_ctl,
    #[cfg(target_os = "linux")]
    epoll_wait,
    #[cfg(target_os = "linux")]
    io_uring_enter,
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    kevent,
    #[cfg(windows)]
    iocp,
    recv,
    recvfrom,
    read,
    pread,
    readv,
    preadv,
    recvmsg,
    connect,
    listen,
    accept,
    shutdown,
    close,
    socket,
    send,
    sendto,
    write,
    pwrite,
    writev,
    pwritev,
    sendmsg,
    fsync,
    renameat,
    #[cfg(target_os = "linux")]
    renameat2,
    mkdirat,
    openat,
}

impl_display_by_debug!(Syscall);

impl<'s> From<Syscall> for &'s str {
    fn from(val: Syscall) -> Self {
        match val {
            #[cfg(windows)]
            Syscall::Sleep => "Sleep",
            Syscall::sleep => "sleep",
            Syscall::usleep => "usleep",
            Syscall::nanosleep => "nanosleep",
            Syscall::poll => "poll",
            Syscall::select => "select",
            #[cfg(target_os = "linux")]
            Syscall::accept4 => "accept4",
            #[cfg(target_os = "linux")]
            Syscall::epoll_ctl => "epoll_ctl",
            #[cfg(target_os = "linux")]
            Syscall::epoll_wait => "epoll_wait",
            #[cfg(target_os = "linux")]
            Syscall::io_uring_enter => "io_uring_enter",
            #[cfg(any(
                target_os = "macos",
                target_os = "ios",
                target_os = "tvos",
                target_os = "watchos",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "openbsd",
                target_os = "netbsd"
            ))]
            Syscall::kevent => "kevent",
            #[cfg(windows)]
            Syscall::iocp => "iocp",
            Syscall::recv => "recv",
            Syscall::recvfrom => "recvfrom",
            Syscall::read => "read",
            Syscall::pread => "pread",
            Syscall::readv => "readv",
            Syscall::preadv => "preadv",
            Syscall::recvmsg => "recvmsg",
            Syscall::connect => "connect",
            Syscall::listen => "listen",
            Syscall::accept => "accept",
            Syscall::shutdown => "shutdown",
            Syscall::close => "close",
            Syscall::socket => "socket",
            Syscall::send => "send",
            Syscall::sendto => "sendto",
            Syscall::write => "write",
            Syscall::pwrite => "pwrite",
            Syscall::writev => "writev",
            Syscall::pwritev => "pwritev",
            Syscall::sendmsg => "sendmsg",
            Syscall::fsync => "fsync",
            Syscall::renameat => "renameat",
            #[cfg(target_os = "linux")]
            Syscall::renameat2 => "renameat2",
            Syscall::mkdirat => "mkdirat",
            Syscall::openat => "openat",
        }
    }
}

/// Enums used to describe syscall state
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SyscallState {
    ///执行中
    Executing,
    ///被挂起到指定时间后继续执行，参数为时间戳
    Suspend(u64),
    ///到指定时间戳后回来，期间系统调用可能没执行完毕
    ///对于sleep系列，这个状态表示正常完成
    Timeout,
    ///系统调用回调成功
    Callback,
}

impl_display_by_debug!(SyscallState);
