#[cfg(target_os = "linux")]
use std::ffi::c_int;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Constants.
pub mod constants;

/// Check <https://www.rustwiki.org.cn/en/reference/introduction.html> for help information.
pub(crate) mod macros;

/// `BeanFactory` impls.
pub mod beans;

/// `TimerList` impls.
pub mod timer;

/// Suppose a thread in a work-stealing scheduler is idle and looking for the next task to run. To
/// find an available task, it might do the following:
///
/// 1. Try popping one task from the local worker queue.
/// 2. Try popping and stealing tasks from another local worker queue.
/// 3. Try popping and stealing a batch of tasks from the global injector queue.
///
/// A queue implementation of work-stealing strategy:
///
/// # Examples
///
/// ```
/// use open_coroutine_core::common::work_steal::WorkStealQueue;
///
/// let queue = WorkStealQueue::new(2, 64);
/// queue.push(6);
/// queue.push(7);
///
/// let local0 = queue.local_queue();
/// local0.push_back(2);
/// local0.push_back(3);
/// local0.push_back(4);
/// local0.push_back(5);
///
/// let local1 = queue.local_queue();
/// local1.push_back(0);
/// local1.push_back(1);
/// for i in 0..8 {
///     assert_eq!(local1.pop_front(), Some(i));
/// }
/// assert_eq!(local0.pop_front(), None);
/// assert_eq!(local1.pop_front(), None);
/// assert_eq!(queue.pop(), None);
/// ```
///
pub mod work_steal;

#[cfg(target_os = "linux")]
extern "C" {
    fn linux_version_code() -> c_int;
}

/// Get linux kernel version number.
#[must_use]
#[cfg(target_os = "linux")]
pub fn kernel_version(major: c_int, patchlevel: c_int, sublevel: c_int) -> c_int {
    ((major) << 16) + ((patchlevel) << 8) + if (sublevel) > 255 { 255 } else { sublevel }
}

/// Get current linux kernel version number.
#[must_use]
#[cfg(target_os = "linux")]
pub fn current_kernel_version() -> c_int {
    unsafe { linux_version_code() }
}

/// get the current wall clock in ns
///
/// # Panics
/// if the time is before `UNIX_EPOCH`
#[must_use]
pub fn now() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("1970-01-01 00:00:00 UTC was {} seconds ago!")
            .as_nanos(),
    )
    .unwrap_or(u64::MAX)
}

/// current ns time add `dur`.
#[must_use]
pub fn get_timeout_time(dur: Duration) -> u64 {
    u64::try_from(dur.as_nanos())
        .map(|d| d.saturating_add(now()))
        .unwrap_or(u64::MAX)
}

/// Make the total time into slices.
#[must_use]
pub fn get_slices(total: Duration, slice: Duration) -> Vec<Duration> {
    let mut result = Vec::new();
    if Duration::ZERO == total {
        return result;
    }
    let mut left_total = total;
    while left_total > slice {
        result.push(slice);
        if let Some(new_left_total) = left_total.checked_sub(slice) {
            left_total = new_left_total;
        }
    }
    result.push(left_total);
    result
}

/// Get the page size of this system.
pub fn page_size() -> usize {
    static PAGE_SIZE: AtomicUsize = AtomicUsize::new(0);
    let mut ret = PAGE_SIZE.load(Ordering::Relaxed);
    if ret == 0 {
        unsafe {
            cfg_if::cfg_if! {
                if #[cfg(windows)] {
                    let mut info = std::mem::zeroed();
                    windows_sys::Win32::System::SystemInformation::GetSystemInfo(&mut info);
                    ret = usize::try_from(info.dwPageSize).expect("get page size failed");
                } else {
                    ret = usize::try_from(libc::sysconf(libc::_SC_PAGESIZE)).expect("get page size failed");
                }
            }
        }
        PAGE_SIZE.store(ret, Ordering::Relaxed);
    }
    ret
}

#[allow(missing_docs)]
#[repr(C)]
#[derive(Debug, Default)]
pub struct CondvarBlocker {
    mutex: std::sync::Mutex<()>,
    condvar: std::sync::Condvar,
}

impl CondvarBlocker {
    /// Block current thread for a while.
    pub fn block(&self, dur: Duration) {
        _ = self
            .condvar
            .wait_timeout(self.mutex.lock().expect("lock failed"), dur);
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn test() {
        assert!(current_kernel_version() > kernel_version(2, 7, 0))
    }
}
