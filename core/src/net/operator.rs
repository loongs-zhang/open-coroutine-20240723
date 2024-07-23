use derivative::Derivative;
use io_uring::opcode::AsyncCancel;
use io_uring::squeue::Entry;
use io_uring::{IoUring, Probe};
use once_cell::sync::Lazy;
use std::collections::VecDeque;
use std::io::{Error, ErrorKind};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

static SUPPORT: Lazy<bool> =
    Lazy::new(|| crate::common::current_kernel_version() >= crate::common::kernel_version(5, 6, 0));

#[must_use]
pub fn support_io_uring() -> bool {
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
    ( $struct_name:ident, $opcode:ident, $impls:expr ) => {{
        static $struct_name: Lazy<bool> = once_cell::sync::Lazy::new(|| {
            if $crate::net::operator::support_io_uring() {
                return PROBE.is_supported($opcode::CODE);
            }
            false
        });
        if $struct_name {
            let entry = $impls;
            return self.push_sq(entry);
        }
        Err(Error::new(ErrorKind::Unsupported, "unsupported"))
    }};
}

#[repr(C)]
#[derive(Derivative)]
#[derivative(Debug)]
pub struct Operator<'o> {
    #[derivative(Debug = "ignore")]
    inner: IoUring,
    entering: AtomicBool,
    backlog: Mutex<VecDeque<&'o Entry>>,
}

impl Operator<'_> {
    pub fn new(_cpu: u32) -> std::io::Result<Self> {
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

    pub fn async_cancel(&self, user_data: usize) -> std::io::Result<()> {
        support!(
            SUPPORT_ASYNC_CANCEL,
            AsyncCancel,
            AsyncCancel::new(user_data as u64)
                .build()
                .user_data(user_data as u64)
        )
    }
}
