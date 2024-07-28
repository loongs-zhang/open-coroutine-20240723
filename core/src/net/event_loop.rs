use crate::common::beans::BeanFactory;
use crate::common::constants::PoolState;
use crate::common::traits::Current;
use crate::net::selector::{Event, Events, Poller, Selector};
use crate::{impl_current_for, impl_display_by_debug};
use crossbeam_utils::atomic::AtomicCell;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::io::{Error, ErrorKind};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

cfg_if::cfg_if! {
    if #[cfg(all(target_os = "linux", feature = "io_uring"))] {
        use libc::{epoll_event, iovec, msghdr, off_t, size_t, sockaddr, socklen_t, ssize_t};
        use dashmap::DashMap;
    }
}

#[repr(C)]
#[derive(Debug)]
pub(super) struct EventLoop<'e> {
    //状态
    state: AtomicCell<PoolState>,
    stop: Arc<(Mutex<bool>, Condvar)>,
    shared_stop: Arc<(Mutex<AtomicUsize>, Condvar)>,
    cpu: usize,
    #[cfg(all(target_os = "linux", feature = "io_uring"))]
    operator: crate::net::operator::Operator<'e>,
    #[cfg(all(target_os = "linux", feature = "io_uring"))]
    result_table: DashMap<usize, ssize_t>,
    selector: Poller,
    // todo remove this when co_pool is implemented
    phantom_data: PhantomData<&'e EventLoop<'e>>,
}

impl EventLoop<'_> {
    pub(crate) fn get_name(&self) -> String {
        format!("{}", self.cpu)
    }

    fn state(&self) -> PoolState {
        self.state.load()
    }

    fn stopping(&self) -> std::io::Result<PoolState> {
        if PoolState::Stopped == self.state() {
            return Err(Error::new(ErrorKind::Other, "unexpect state"));
        }
        Ok(self.state.swap(PoolState::Stopping))
    }

    fn stopped(&self) -> std::io::Result<PoolState> {
        if PoolState::Stopping == self.state() {
            return Ok(self.state.swap(PoolState::Stopped));
        }
        Err(Error::new(ErrorKind::Other, "unexpect state"))
    }
}

impl<'e> EventLoop<'e> {
    pub(super) fn new(
        cpu: usize,
        shared_stop: Arc<(Mutex<AtomicUsize>, Condvar)>,
    ) -> std::io::Result<Self> {
        Ok(EventLoop {
            state: AtomicCell::new(PoolState::Running),
            stop: Arc::new((Mutex::new(false), Condvar::new())),
            shared_stop,
            cpu,
            #[cfg(all(target_os = "linux", feature = "io_uring"))]
            operator: crate::net::operator::Operator::new(cpu)?,
            #[cfg(all(target_os = "linux", feature = "io_uring"))]
            result_table: DashMap::new(),
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

    #[cfg(all(target_os = "linux", feature = "io_uring"))]
    pub(super) fn try_get_syscall_result(&self, token: usize) -> Option<ssize_t> {
        self.result_table.remove(&token).map(|(_, result)| result)
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
                    .result_table
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

    pub(super) fn start(self) -> std::io::Result<Arc<Self>>
    where
        'e: 'static,
    {
        // init stop flag
        {
            let (lock, cvar) = &*self.stop;
            let mut pending = lock.lock().expect("lock failed");
            *pending = true;
            cvar.notify_one();
        }
        let thread_name = self.get_thread_name();
        let bean_name = self.get_name().to_string().leak();
        let bean_name_in_thread = self.get_name().to_string().leak();
        BeanFactory::init_bean(bean_name, self);
        BeanFactory::init_bean(
            &thread_name,
            std::thread::Builder::new()
                .name(thread_name.clone())
                .spawn(move || {
                    let consumer =
                        unsafe { BeanFactory::get_mut_bean::<Self>(bean_name_in_thread) }
                            .unwrap_or_else(|| panic!("bean {bean_name_in_thread} not exist !"));
                    {
                        let (lock, cvar) = &*consumer.shared_stop.clone();
                        let started = lock.lock().expect("lock failed");
                        _ = started.fetch_add(1, Ordering::Release);
                        cvar.notify_one();
                    }
                    // thread per core
                    eprintln!(
                        "{} has started, pin:{}",
                        consumer.get_name(),
                        core_affinity::set_for_current(core_affinity::CoreId { id: consumer.cpu })
                    );
                    Self::init_current(consumer);
                    while PoolState::Running == consumer.state()
                    // || !consumer.is_empty()
                    // || consumer.get_running_size() > 0
                    {
                        _ = consumer.wait_event(Some(Duration::from_millis(10)));
                    }
                    // notify stop flags
                    {
                        let (lock, cvar) = &*consumer.stop.clone();
                        let mut pending = lock.lock().expect("lock failed");
                        *pending = false;
                        cvar.notify_one();
                    }
                    {
                        let (lock, cvar) = &*consumer.shared_stop.clone();
                        let started = lock.lock().expect("lock failed");
                        _ = started.fetch_sub(1, Ordering::Release);
                        cvar.notify_one();
                    }
                    Self::clean_current();
                    eprintln!("{} has exited", consumer.get_name());
                })?,
        );
        unsafe {
            Ok(Arc::from_raw(
                BeanFactory::get_bean::<Self>(bean_name)
                    .unwrap_or_else(|| panic!("bean {bean_name} not exist !")),
            ))
        }
    }

    fn get_thread_name(&self) -> String {
        format!("{}-thread", self.get_name())
    }

    // pub(super) fn stop_sync(&mut self, wait_time: Duration) -> std::io::Result<()> {
    //     match self.state() {
    //         PoolState::Running => {
    //             assert_eq!(PoolState::Running, self.stopping()?);
    //             let mut left = wait_time;
    //             let once = Duration::from_millis(10);
    //             loop {
    //                 if left.is_zero() {
    //                     return Err(Error::new(ErrorKind::TimedOut, "stop timeout !"));
    //                 }
    //                 self.wait_event(Some(left.min(once)))?;
    //                 if self.pool.is_empty() && self.pool.get_running_size() == 0 {
    //                     assert_eq!(PoolState::Stopping, self.stopped()?);
    //                     return Ok(());
    //                 }
    //                 left = left.saturating_sub(once);
    //             }
    //         }
    //         PoolState::Stopping => Err(Error::new(ErrorKind::Other, "should never happens")),
    //         PoolState::Stopped => Ok(()),
    //     }
    // }

    pub(super) fn stop(&self, wait_time: Duration) -> std::io::Result<()> {
        match self.state() {
            PoolState::Running => {
                if BeanFactory::remove_bean::<JoinHandle<()>>(&self.get_thread_name()).is_some() {
                    assert_eq!(PoolState::Running, self.stopping()?);
                    //开启了单独的线程
                    let (lock, cvar) = &*self.stop;
                    let result = cvar
                        .wait_timeout_while(lock.lock().unwrap(), wait_time, |&mut pending| pending)
                        .unwrap();
                    if result.1.timed_out() {
                        return Err(Error::new(ErrorKind::TimedOut, "stop timeout !"));
                    }
                    assert_eq!(PoolState::Stopping, self.stopped()?);
                }
                Ok(())
            }
            PoolState::Stopping => Err(Error::new(ErrorKind::Other, "should never happens")),
            PoolState::Stopped => Ok(()),
        }
    }
}

impl_current_for!(EVENT_LOOP, EventLoop<'e>);

impl_display_by_debug!(EventLoop<'e>);

macro_rules! impl_io_uring {
    ( $syscall: ident($($arg: ident : $arg_type: ty),*) -> $result: ty ) => {
        #[cfg(all(target_os = "linux", feature = "io_uring"))]
        impl EventLoop<'_> {
            pub(super) fn $syscall(
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
impl_io_uring!(shutdown(fd: c_int, how: c_int) -> c_int);
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
