use derivative::Derivative;
use io_uring::opcode::{
    Accept, AsyncCancel, Close, Connect, EpollCtl, Fsync, MkDirAt, OpenAt, PollAdd, PollRemove,
    Read, Readv, Recv, RecvMsg, RenameAt, Send, SendMsg, SendZc, Shutdown, Socket, Timeout,
    TimeoutRemove, TimeoutUpdate, Write, Writev,
};
use io_uring::squeue::Entry;
use io_uring::types::{epoll_event, Fd, Timespec};
use io_uring::{CompletionQueue, IoUring, Probe};
use libc::{
    c_char, c_int, c_uint, c_void, iovec, mode_t, msghdr, off_t, size_t, sockaddr, socklen_t, EBUSY,
};
use once_cell::sync::Lazy;
use std::collections::VecDeque;
use std::io::{Error, ErrorKind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

static SUPPORT: Lazy<bool> =
    Lazy::new(|| crate::common::current_kernel_version() >= crate::common::kernel_version(5, 6, 0));

#[must_use]
fn support_io_uring() -> bool {
    *SUPPORT
}

static PROBE: Lazy<Probe> = Lazy::new(|| {
    let mut probe = Probe::new();
    if let Ok(io_uring) = IoUring::new(2) {
        if let Ok(()) = io_uring.submitter().register_probe(&mut probe) {
            return probe;
        }
    }
    panic!("probe init failed !")
});

// check https://www.rustwiki.org.cn/en/reference/introduction.html for help information
macro_rules! support {
    ( $self:ident, $struct_name:ident, $opcode:ident, $impls:expr ) => {
        return {
            static $struct_name: Lazy<bool> = once_cell::sync::Lazy::new(|| {
                if $crate::net::operator::support_io_uring() {
                    return PROBE.is_supported($opcode::CODE);
                }
                false
            });
            if *$struct_name {
                return $self.push_sq($impls);
            }
            Err(Error::new(ErrorKind::Unsupported, "unsupported"))
        }
    };
}

#[repr(C)]
#[derive(Derivative)]
#[derivative(Debug)]
pub(crate) struct Operator<'o> {
    #[derivative(Debug = "ignore")]
    inner: IoUring,
    entering: AtomicBool,
    backlog: Mutex<VecDeque<&'o Entry>>,
}

impl Operator<'_> {
    pub(crate) fn new(_cpu: u32) -> std::io::Result<Self> {
        Ok(Operator {
            inner: IoUring::builder().build(1024)?,
            entering: AtomicBool::new(false),
            backlog: Mutex::new(VecDeque::new()),
        })
    }

    fn push_sq(&self, entry: Entry) -> std::io::Result<()> {
        let entry = Box::leak(Box::new(entry));
        if unsafe { self.inner.submission_shared().push(entry).is_err() } {
            self.backlog.lock().unwrap().push_back(entry);
        }
        self.inner.submit().map(|_| ())
    }

    pub(crate) fn select(
        &self,
        timeout: Option<Duration>,
    ) -> std::io::Result<(usize, CompletionQueue)> {
        if support_io_uring() {
            if self
                .entering
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
            {
                return Ok((0, unsafe { self.inner.completion_shared() }));
            }
            let result = self.do_select(timeout);
            self.entering.store(false, Ordering::Release);
            return result;
        }
        Err(Error::new(ErrorKind::Unsupported, "unsupported"))
    }

    fn do_select(&self, _timeout: Option<Duration>) -> std::io::Result<(usize, CompletionQueue)> {
        let mut cq = unsafe { self.inner.completion_shared() };
        let count = match self.inner.submit_and_wait(0) {
            Ok(count) => count,
            Err(err) => {
                if err.raw_os_error() == Some(EBUSY) {
                    0
                } else {
                    return Err(err);
                }
            }
        };
        cq.sync();

        // clean backlog
        let mut sq = unsafe { self.inner.submission_shared() };
        loop {
            if sq.is_full() {
                match self.inner.submit() {
                    Ok(_) => (),
                    Err(err) => {
                        if err.raw_os_error() == Some(EBUSY) {
                            break;
                        }
                        return Err(err);
                    }
                }
            }
            sq.sync();

            let mut backlog = self.backlog.lock().unwrap();
            match backlog.pop_front() {
                Some(sqe) => {
                    if unsafe { sq.push(sqe).is_err() } {
                        backlog.push_front(sqe);
                        break;
                    }
                }
                None => break,
            }
        }
        Ok((count, cq))
    }

    pub(crate) fn async_cancel(&self, user_data: usize) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_ASYNC_CANCEL,
            AsyncCancel,
            AsyncCancel::new(user_data as u64)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn epoll_ctl(
        &self,
        user_data: usize,
        epfd: c_int,
        op: c_int,
        fd: c_int,
        event: *mut libc::epoll_event,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_EPOLL_CTL,
            EpollCtl,
            EpollCtl::new(
                Fd(epfd),
                Fd(fd),
                op,
                event.cast_const().cast::<epoll_event>(),
            )
            .build()
            .user_data(user_data as u64)
        )
    }

    pub(crate) fn poll_add(
        &self,
        user_data: usize,
        fd: c_int,
        flags: c_int,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_POLL_ADD,
            PollAdd,
            PollAdd::new(Fd(fd), flags as u32)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn poll_remove(&self, user_data: usize) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_POLL_REMOVE,
            PollRemove,
            PollRemove::new(user_data as u64)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn timeout_add(
        &self,
        user_data: usize,
        timeout: Option<Duration>,
    ) -> std::io::Result<()> {
        if let Some(duration) = timeout {
            let timeout = Timespec::new()
                .sec(duration.as_secs())
                .nsec(duration.subsec_nanos());
            support!(
                self,
                SUPPORT_TIMEOUT_ADD,
                Timeout,
                Timeout::new(&timeout).build().user_data(user_data as u64)
            )
        }
        Ok(())
    }

    pub(crate) fn timeout_update(
        &self,
        user_data: usize,
        timeout: Option<Duration>,
    ) -> std::io::Result<()> {
        if let Some(duration) = timeout {
            let timeout = Timespec::new()
                .sec(duration.as_secs())
                .nsec(duration.subsec_nanos());
            support!(
                self,
                SUPPORT_TIMEOUT_UPDATE,
                TimeoutUpdate,
                TimeoutUpdate::new(user_data as u64, &timeout)
                    .build()
                    .user_data(user_data as u64)
            )
        }
        self.timeout_remove(user_data)
    }

    pub(crate) fn timeout_remove(&self, user_data: usize) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_TIMEOUT_REMOVE,
            TimeoutRemove,
            TimeoutRemove::new(user_data as u64).build()
        )
    }

    pub(crate) fn openat(
        &self,
        user_data: usize,
        dir_fd: c_int,
        pathname: *const c_char,
        flags: c_int,
        mode: mode_t,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_OPENAT,
            OpenAt,
            OpenAt::new(Fd(dir_fd), pathname)
                .flags(flags)
                .mode(mode)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn mkdirat(
        &self,
        user_data: usize,
        dir_fd: c_int,
        pathname: *const c_char,
        mode: mode_t,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_MK_DIR_AT,
            MkDirAt,
            MkDirAt::new(Fd(dir_fd), pathname)
                .mode(mode)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn renameat(
        &self,
        user_data: usize,
        old_dir_fd: c_int,
        old_path: *const c_char,
        new_dir_fd: c_int,
        new_path: *const c_char,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_RENAME_AT,
            RenameAt,
            RenameAt::new(Fd(old_dir_fd), old_path, Fd(new_dir_fd), new_path)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn renameat2(
        &self,
        user_data: usize,
        old_dir_fd: c_int,
        old_path: *const c_char,
        new_dir_fd: c_int,
        new_path: *const c_char,
        flags: c_uint,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_RENAME_AT,
            RenameAt,
            RenameAt::new(Fd(old_dir_fd), old_path, Fd(new_dir_fd), new_path)
                .flags(flags)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn fsync(&self, user_data: usize, fd: c_int) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_FSYNC,
            Fsync,
            Fsync::new(Fd(fd)).build().user_data(user_data as u64)
        )
    }

    pub(crate) fn socket(
        &self,
        user_data: usize,
        domain: c_int,
        ty: c_int,
        protocol: c_int,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_SOCKET,
            Socket,
            Socket::new(domain, ty, protocol)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn accept(
        &self,
        user_data: usize,
        fd: c_int,
        address: *mut sockaddr,
        address_len: *mut socklen_t,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_ACCEPT,
            Accept,
            Accept::new(Fd(fd), address, address_len)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn accept4(
        &self,
        user_data: usize,
        fd: c_int,
        addr: *mut sockaddr,
        len: *mut socklen_t,
        flg: c_int,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_ACCEPT,
            Accept,
            Accept::new(Fd(fd), addr, len)
                .flags(flg)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn connect(
        &self,
        user_data: usize,
        fd: c_int,
        address: *const sockaddr,
        len: socklen_t,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_CONNECT,
            Connect,
            Connect::new(Fd(fd), address, len)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn shutdown(&self, user_data: usize, fd: c_int, how: c_int) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_SHUTDOWN,
            Shutdown,
            Shutdown::new(Fd(fd), how)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn close(&self, user_data: usize, fd: c_int) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_CLOSE,
            Close,
            Close::new(Fd(fd)).build().user_data(user_data as u64)
        )
    }

    pub(crate) fn recv(
        &self,
        user_data: usize,
        fd: c_int,
        buf: *mut c_void,
        len: size_t,
        flags: c_int,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_RECV,
            Recv,
            Recv::new(Fd(fd), buf.cast::<u8>(), len as u32)
                .flags(flags)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn read(
        &self,
        user_data: usize,
        fd: c_int,
        buf: *mut c_void,
        count: size_t,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_READ,
            Read,
            Read::new(Fd(fd), buf.cast::<u8>(), count as u32)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn pread(
        &self,
        user_data: usize,
        fd: c_int,
        buf: *mut c_void,
        count: size_t,
        offset: off_t,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_READ,
            Read,
            Read::new(Fd(fd), buf.cast::<u8>(), count as u32)
                .offset(offset as u64)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn readv(
        &self,
        user_data: usize,
        fd: c_int,
        iov: *const iovec,
        iovcnt: c_int,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_READV,
            Readv,
            Readv::new(Fd(fd), iov, iovcnt as u32)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn preadv(
        &self,
        user_data: usize,
        fd: c_int,
        iov: *const iovec,
        iovcnt: c_int,
        offset: off_t,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_READV,
            Readv,
            Readv::new(Fd(fd), iov, iovcnt as u32)
                .offset(offset as u64)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn recvmsg(
        &self,
        user_data: usize,
        fd: c_int,
        msg: *mut msghdr,
        flags: c_int,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_RECVMSG,
            RecvMsg,
            RecvMsg::new(Fd(fd), msg)
                .flags(flags as u32)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn send(
        &self,
        user_data: usize,
        fd: c_int,
        buf: *const c_void,
        len: size_t,
        flags: c_int,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_SEND,
            Send,
            Send::new(Fd(fd), buf.cast::<u8>(), len as u32)
                .flags(flags)
                .build()
                .user_data(user_data as u64)
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn sendto(
        &self,
        user_data: usize,
        fd: c_int,
        buf: *const c_void,
        len: size_t,
        flags: c_int,
        addr: *const sockaddr,
        addrlen: socklen_t,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_SEND_ZC,
            SendZc,
            SendZc::new(Fd(fd), buf.cast::<u8>(), len as u32)
                .flags(flags)
                .dest_addr(addr)
                .dest_addr_len(addrlen)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn write(
        &self,
        user_data: usize,
        fd: c_int,
        buf: *const c_void,
        count: size_t,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_WRITE,
            Write,
            Write::new(Fd(fd), buf.cast::<u8>(), count as u32)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn pwrite(
        &self,
        user_data: usize,
        fd: c_int,
        buf: *const c_void,
        count: size_t,
        offset: off_t,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_WRITE,
            Write,
            Write::new(Fd(fd), buf.cast::<u8>(), count as u32)
                .offset(offset as u64)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn writev(
        &self,
        user_data: usize,
        fd: c_int,
        iov: *const iovec,
        iovcnt: c_int,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_WRITEV,
            Writev,
            Writev::new(Fd(fd), iov, iovcnt as u32)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn pwritev(
        &self,
        user_data: usize,
        fd: c_int,
        iov: *const iovec,
        iovcnt: c_int,
        offset: off_t,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_WRITEV,
            Writev,
            Writev::new(Fd(fd), iov, iovcnt as u32)
                .offset(offset as u64)
                .build()
                .user_data(user_data as u64)
        )
    }

    pub(crate) fn sendmsg(
        &self,
        user_data: usize,
        fd: c_int,
        msg: *const msghdr,
        flags: c_int,
    ) -> std::io::Result<()> {
        support!(
            self,
            SUPPORT_SENDMSG,
            SendMsg,
            SendMsg::new(Fd(fd), msg)
                .flags(flags as u32)
                .build()
                .user_data(user_data as u64)
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::net::operator::Operator;
    use io_uring::{opcode, squeue, types, IoUring, SubmissionQueue};
    use slab::Slab;
    use std::collections::VecDeque;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
    use std::os::unix::io::{AsRawFd, RawFd};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use std::{io, ptr};

    #[derive(Clone, Debug)]
    enum Token {
        Accept,
        Poll {
            fd: RawFd,
        },
        Read {
            fd: RawFd,
            buf_index: usize,
        },
        Write {
            fd: RawFd,
            buf_index: usize,
            offset: usize,
            len: usize,
        },
    }

    struct AcceptCount {
        entry: squeue::Entry,
        count: usize,
    }

    impl AcceptCount {
        fn new(fd: RawFd, token: usize, count: usize) -> AcceptCount {
            AcceptCount {
                entry: opcode::Accept::new(types::Fd(fd), ptr::null_mut(), ptr::null_mut())
                    .build()
                    .user_data(token as _),
                count,
            }
        }

        fn push_to(&mut self, sq: &mut SubmissionQueue<'_>) {
            while self.count > 0 {
                unsafe {
                    match sq.push(&self.entry) {
                        Ok(_) => self.count -= 1,
                        Err(_) => break,
                    }
                }
            }

            sq.sync();
        }
    }

    fn crate_server(port: u16, server_started: Arc<AtomicBool>) -> anyhow::Result<()> {
        let mut ring: IoUring = IoUring::builder()
            .setup_sqpoll(1000)
            .setup_sqpoll_cpu(0)
            .build(1024)?;
        let listener = TcpListener::bind(("127.0.0.1", port))?;

        let mut backlog = VecDeque::new();
        let mut bufpool = Vec::with_capacity(64);
        let mut buf_alloc = Slab::with_capacity(64);
        let mut token_alloc = Slab::with_capacity(64);

        println!("listen {}", listener.local_addr()?);
        server_started.store(true, Ordering::Release);

        let (submitter, mut sq, mut cq) = ring.split();

        let mut accept =
            AcceptCount::new(listener.as_raw_fd(), token_alloc.insert(Token::Accept), 1);

        accept.push_to(&mut sq);

        loop {
            match submitter.submit_and_wait(1) {
                Ok(_) => (),
                Err(ref err) if err.raw_os_error() == Some(libc::EBUSY) => (),
                Err(err) => return Err(err.into()),
            }
            cq.sync();

            // clean backlog
            loop {
                if sq.is_full() {
                    match submitter.submit() {
                        Ok(_) => (),
                        Err(ref err) if err.raw_os_error() == Some(libc::EBUSY) => break,
                        Err(err) => return Err(err.into()),
                    }
                }
                sq.sync();

                match backlog.pop_front() {
                    Some(sqe) => unsafe {
                        let _ = sq.push(&sqe);
                    },
                    None => break,
                }
            }

            accept.push_to(&mut sq);

            for cqe in &mut cq {
                let ret = cqe.result();
                let token_index = cqe.user_data() as usize;

                if ret < 0 {
                    eprintln!(
                        "token {:?} error: {:?}",
                        token_alloc.get(token_index),
                        io::Error::from_raw_os_error(-ret)
                    );
                    continue;
                }

                let token = &mut token_alloc[token_index];
                match token.clone() {
                    Token::Accept => {
                        println!("accept");

                        accept.count += 1;

                        let fd = ret;
                        let poll_token = token_alloc.insert(Token::Poll { fd });

                        let poll_e = opcode::PollAdd::new(types::Fd(fd), libc::POLLIN as _)
                            .build()
                            .user_data(poll_token as _);

                        unsafe {
                            if sq.push(&poll_e).is_err() {
                                backlog.push_back(poll_e);
                            }
                        }
                    }
                    Token::Poll { fd } => {
                        let (buf_index, buf) = match bufpool.pop() {
                            Some(buf_index) => (buf_index, &mut buf_alloc[buf_index]),
                            None => {
                                let buf = vec![0u8; 2048].into_boxed_slice();
                                let buf_entry = buf_alloc.vacant_entry();
                                let buf_index = buf_entry.key();
                                (buf_index, buf_entry.insert(buf))
                            }
                        };

                        *token = Token::Read { fd, buf_index };

                        let read_e =
                            opcode::Recv::new(types::Fd(fd), buf.as_mut_ptr(), buf.len() as _)
                                .build()
                                .user_data(token_index as _);

                        unsafe {
                            if sq.push(&read_e).is_err() {
                                backlog.push_back(read_e);
                            }
                        }
                    }
                    Token::Read { fd, buf_index } => {
                        if ret == 0 {
                            bufpool.push(buf_index);
                            _ = token_alloc.remove(token_index);
                            println!("shutdown connection");
                            unsafe { _ = libc::close(fd) };

                            println!("Server closed");
                            return Ok(());
                        } else {
                            let len = ret as usize;
                            let buf = &buf_alloc[buf_index];

                            *token = Token::Write {
                                fd,
                                buf_index,
                                len,
                                offset: 0,
                            };

                            let write_e = opcode::Send::new(types::Fd(fd), buf.as_ptr(), len as _)
                                .build()
                                .user_data(token_index as _);

                            unsafe {
                                if sq.push(&write_e).is_err() {
                                    backlog.push_back(write_e);
                                }
                            }
                        }
                    }
                    Token::Write {
                        fd,
                        buf_index,
                        offset,
                        len,
                    } => {
                        let write_len = ret as usize;

                        let entry = if offset + write_len >= len {
                            bufpool.push(buf_index);

                            *token = Token::Poll { fd };

                            opcode::PollAdd::new(types::Fd(fd), libc::POLLIN as _)
                                .build()
                                .user_data(token_index as _)
                        } else {
                            let offset = offset + write_len;
                            let len = len - offset;

                            let buf = &buf_alloc[buf_index][offset..];

                            *token = Token::Write {
                                fd,
                                buf_index,
                                offset,
                                len,
                            };

                            opcode::Write::new(types::Fd(fd), buf.as_ptr(), len as _)
                                .build()
                                .user_data(token_index as _)
                        };

                        unsafe {
                            if sq.push(&entry).is_err() {
                                backlog.push_back(entry);
                            }
                        }
                    }
                }
            }
        }
    }

    fn crate_client(port: u16, server_started: Arc<AtomicBool>) {
        //等服务端起来
        while !server_started.load(Ordering::Acquire) {}
        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);
        let mut stream = TcpStream::connect_timeout(&socket, Duration::from_secs(3))
            .unwrap_or_else(|_| panic!("connect to 127.0.0.1:{port} failed !"));
        let mut data: [u8; 512] = [b'1'; 512];
        data[511] = b'\n';
        let mut buffer: Vec<u8> = Vec::with_capacity(512);
        for _ in 0..3 {
            //写入stream流，如果写入失败，提示"写入失败"
            assert_eq!(512, stream.write(&data).expect("Failed to write!"));
            print!("Client Send: {}", String::from_utf8_lossy(&data[..]));

            let mut reader = BufReader::new(&stream);
            //一直读到换行为止（b'\n'中的b表示字节），读到buffer里面
            assert_eq!(
                512,
                reader
                    .read_until(b'\n', &mut buffer)
                    .expect("Failed to read into buffer")
            );
            print!("Client Received: {}", String::from_utf8_lossy(&buffer[..]));
            assert_eq!(&data, &buffer as &[u8]);
            buffer.clear();
        }
        //发送终止符
        assert_eq!(1, stream.write(&[b'e']).expect("Failed to write!"));
        println!("client closed");
    }

    #[test]
    fn original() -> anyhow::Result<()> {
        let port = 7060;
        let server_started = Arc::new(AtomicBool::new(false));
        let clone = server_started.clone();
        let handle = std::thread::spawn(move || crate_server(port, clone));
        std::thread::spawn(move || crate_client(port, server_started))
            .join()
            .expect("client has error");
        handle.join().expect("server has error")
    }

    fn crate_server2(port: u16, server_started: Arc<AtomicBool>) -> anyhow::Result<()> {
        let operator = Operator::new(0)?;
        let listener = TcpListener::bind(("127.0.0.1", port))?;

        let mut bufpool = Vec::with_capacity(64);
        let mut buf_alloc = Slab::with_capacity(64);
        let mut token_alloc = Slab::with_capacity(64);

        println!("listen {}", listener.local_addr()?);
        server_started.store(true, Ordering::Release);

        operator.accept(
            token_alloc.insert(Token::Accept),
            listener.as_raw_fd(),
            ptr::null_mut(),
            ptr::null_mut(),
        )?;

        loop {
            let mut r = operator.select(None)?;

            for cqe in &mut r.1 {
                let ret = cqe.result();
                let token_index = cqe.user_data() as usize;

                if ret < 0 {
                    eprintln!(
                        "token {:?} error: {:?}",
                        token_alloc.get(token_index),
                        io::Error::from_raw_os_error(-ret)
                    );
                    continue;
                }

                let token = &mut token_alloc[token_index];
                match token.clone() {
                    Token::Accept => {
                        println!("accept");

                        let fd = ret;
                        let poll_token = token_alloc.insert(Token::Poll { fd });

                        operator.poll_add(poll_token, fd, libc::POLLIN as _)?;
                    }
                    Token::Poll { fd } => {
                        let (buf_index, buf) = match bufpool.pop() {
                            Some(buf_index) => (buf_index, &mut buf_alloc[buf_index]),
                            None => {
                                let buf = vec![0u8; 2048].into_boxed_slice();
                                let buf_entry = buf_alloc.vacant_entry();
                                let buf_index = buf_entry.key();
                                (buf_index, buf_entry.insert(buf))
                            }
                        };

                        *token = Token::Read { fd, buf_index };

                        operator.recv(token_index, fd, buf.as_mut_ptr() as _, buf.len(), 0)?;
                    }
                    Token::Read { fd, buf_index } => {
                        if ret == 0 {
                            bufpool.push(buf_index);
                            _ = token_alloc.remove(token_index);
                            println!("shutdown connection");
                            unsafe { _ = libc::close(fd) };

                            println!("Server closed");
                            return Ok(());
                        } else {
                            let len = ret as usize;
                            let buf = &buf_alloc[buf_index];

                            *token = Token::Write {
                                fd,
                                buf_index,
                                len,
                                offset: 0,
                            };

                            operator.send(token_index, fd, buf.as_ptr() as _, len, 0)?;
                        }
                    }
                    Token::Write {
                        fd,
                        buf_index,
                        offset,
                        len,
                    } => {
                        let write_len = ret as usize;

                        if offset + write_len >= len {
                            bufpool.push(buf_index);

                            *token = Token::Poll { fd };

                            operator.poll_add(token_index, fd, libc::POLLIN as _)?;
                        } else {
                            let offset = offset + write_len;
                            let len = len - offset;

                            let buf = &buf_alloc[buf_index][offset..];

                            *token = Token::Write {
                                fd,
                                buf_index,
                                offset,
                                len,
                            };

                            operator.write(token_index, fd, buf.as_ptr() as _, len)?;
                        };
                    }
                }
            }
        }
    }

    #[test]
    fn framework() -> anyhow::Result<()> {
        let port = 7061;
        let server_started = Arc::new(AtomicBool::new(false));
        let clone = server_started.clone();
        let handle = std::thread::spawn(move || crate_server2(port, clone));
        std::thread::spawn(move || crate_client(port, server_started))
            .join()
            .expect("client has error");
        handle.join().expect("server has error")
    }
}
