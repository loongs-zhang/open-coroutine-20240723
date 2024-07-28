#[cfg(target_os = "linux")]
use std::ffi::c_int;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// `BeanFactory` impls.
pub mod beans;

/// Traits.
pub mod traits;

pub(crate) mod macros;

/// Constants.
pub mod constants;

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
