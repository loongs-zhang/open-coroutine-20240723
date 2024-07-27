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
