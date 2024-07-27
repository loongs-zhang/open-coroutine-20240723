#[cfg(target_os = "linux")]
use std::ffi::c_int;

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
