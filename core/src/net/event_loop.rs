use crate::net::selector::{Event, Events, Poller, Selector};
#[cfg(all(target_os = "linux", feature = "io_uring"))]
use libc::{epoll_event, iovec, msghdr, off_t, size_t, sockaddr, socklen_t, ssize_t};
use std::ffi::{c_char, c_int, c_void, CStr};
use std::marker::PhantomData;
use std::time::Duration;

#[repr(C)]
#[derive(Debug)]
pub(super) struct EventLoop<'e> {
    cpu: u32,
    #[cfg(all(target_os = "linux", feature = "io_uring"))]
    operator: crate::net::operator::Operator<'e>,
    #[cfg(all(target_os = "linux", feature = "io_uring"))]
    operate_table: dashmap::DashMap<usize, ssize_t>,
    selector: Poller,
    // todo remove this when co_pool is implemented
    phantom_data: PhantomData<&'e EventLoop<'e>>,
}

impl EventLoop<'_> {
    pub(super) fn new(cpu: u32) -> std::io::Result<Self> {
        Ok(EventLoop {
            cpu,
            #[cfg(all(target_os = "linux", feature = "io_uring"))]
            operator: crate::net::operator::Operator::new(cpu)?,
            #[cfg(all(target_os = "linux", feature = "io_uring"))]
            operate_table: dashmap::DashMap::new(),
            selector: Poller::new()?,
            phantom_data: PhantomData,
        })
    }

    #[allow(trivial_numeric_casts, clippy::cast_possible_truncation)]
    fn token() -> usize {
        //todo coroutine
        unsafe {
            cfg_if::cfg_if! {
                if #[cfg(windows)] {
                    let thread_id = windows_sys::Win32::System::Threading::GetCurrentThread();
                } else {
                    let thread_id = libc::pthread_self();
                }
            }
            thread_id as usize
        }
    }

    pub(super) fn add_read_event(&self, fd: c_int) -> std::io::Result<()> {
        self.selector.add_read_event(fd, EventLoop::token())
    }

    pub(super) fn add_write_event(&self, fd: c_int) -> std::io::Result<()> {
        self.selector.add_write_event(fd, EventLoop::token())
    }

    pub(super) fn del_event(&self, fd: c_int) -> std::io::Result<()> {
        self.selector.del_event(fd)
    }

    pub(super) fn del_read_event(&self, fd: c_int) -> std::io::Result<()> {
        self.selector.del_read_event(fd)
    }

    pub(super) fn del_write_event(&self, fd: c_int) -> std::io::Result<()> {
        self.selector.del_write_event(fd)
    }

    pub(super) fn wait_event(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        //todo
        self.wait_just(timeout)
    }

    pub(super) fn wait_just(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        #[cfg(all(target_os = "linux", feature = "io_uring"))]
        if crate::net::operator::support_io_uring() {
            // use io_uring
            let mut result = self.operator.select(timeout)?;
            for cqe in &mut result.1 {
                let token = cqe.user_data() as usize;
                // resolve completed read/write tasks
                assert!(self
                    .operate_table
                    .insert(token, cqe.result() as ssize_t)
                    .is_none());
                unsafe { self.resume(token) };
            }
        }

        // use epoll/kevent/iocp
        let mut events = Events::with_capacity(1024);
        self.selector.select(&mut events, timeout)?;
        for event in &events {
            let token = event.get_token();
            if event.readable() || event.writable() {
                unsafe { self.resume(token) };
            }
        }
        Ok(())
    }

    #[allow(clippy::unused_self)]
    unsafe fn resume(&self, token: usize) {
        if token == 0 {
            return;
        }
        if let Ok(_co_name) = CStr::from_ptr((token as *const c_void).cast::<c_char>()).to_str() {
            //todo coroutine
        }
    }
}

macro_rules! impl_io_uring {
    ( $syscall: ident($($arg: ident : $arg_type: ty),*) -> $result: ty ) => {
        #[cfg(all(target_os = "linux", feature = "io_uring"))]
        impl EventLoop<'_> {
            pub fn $syscall(
                &self,
                $($arg: $arg_type),*
            ) -> std::io::Result<usize> {
                let token = EventLoop::token();
                self.operator
                    .$syscall(token, $($arg, )*)
                    .map(|()| token)
            }
        }
    }
}

impl_io_uring!(epoll_ctl(epfd: c_int, op: c_int, fd: c_int, event: *mut epoll_event) -> c_int);
impl_io_uring!(socket(domain: c_int, ty: c_int, protocol: c_int) -> c_int);
impl_io_uring!(accept(fd: c_int, addr: *mut sockaddr, len: *mut socklen_t) -> c_int);
impl_io_uring!(accept4(fd: c_int, addr: *mut sockaddr, len: *mut socklen_t, flg: c_int) -> c_int);
impl_io_uring!(connect(fd: c_int, address: *const sockaddr, len: socklen_t) -> c_int);
impl_io_uring!(close(fd: c_int) -> c_int);
impl_io_uring!(recv(fd: c_int, buf: *mut c_void, len: size_t, flags: c_int) -> ssize_t);
impl_io_uring!(read(fd: c_int, buf: *mut c_void, count: size_t) -> ssize_t);
impl_io_uring!(pread(fd: c_int, buf: *mut c_void, count: size_t, offset: off_t) -> ssize_t);
impl_io_uring!(readv(fd: c_int, iov: *const iovec, iovcnt: c_int) -> ssize_t);
impl_io_uring!(preadv(fd: c_int, iov: *const iovec, iovcnt: c_int, offset: off_t) -> ssize_t);
impl_io_uring!(recvmsg(fd: c_int, msg: *mut msghdr, flags: c_int) -> ssize_t);
impl_io_uring!(send(fd: c_int, buf: *const c_void, len: size_t, flags: c_int) -> ssize_t);
impl_io_uring!(sendto(fd: c_int, buf: *const c_void, len: size_t, flags: c_int, addr: *const sockaddr, addrlen: socklen_t) -> ssize_t);
impl_io_uring!(write(fd: c_int, buf: *const c_void, count: size_t) -> ssize_t);
impl_io_uring!(pwrite(fd: c_int, buf: *const c_void, count: size_t, offset: off_t) -> ssize_t);
impl_io_uring!(writev(fd: c_int, iov: *const iovec, iovcnt: c_int) -> ssize_t);
impl_io_uring!(pwritev(fd: c_int, iov: *const iovec, iovcnt: c_int, offset: off_t) -> ssize_t);
impl_io_uring!(sendmsg(fd: c_int, msg: *const msghdr, flags: c_int) -> ssize_t);
