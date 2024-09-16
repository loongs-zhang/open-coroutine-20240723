#![deny(
    // The following are allowed by default lints according to
    // https://doc.rust-lang.org/rustc/lints/listing/allowed-by-default.html
    anonymous_parameters,
    bare_trait_objects,
    // elided_lifetimes_in_paths, // allow anonymous lifetime
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs, // TODO: add documents
    single_use_lifetimes, // TODO: fix lifetime names only used once
    trivial_casts, // TODO: remove trivial casts in code
    trivial_numeric_casts,
    // unreachable_pub, allow clippy::redundant_pub_crate lint instead
    // unsafe_code,
    unstable_features,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications,
    unused_results,
    variant_size_differences,

    warnings, // treat all wanings as errors

    clippy::all,
    // clippy::restriction,
    clippy::pedantic,
    // clippy::nursery, // It's still under development
    clippy::cargo,
    unreachable_pub,
)]
#![allow(
    // Some explicitly allowed Clippy lints, must have clear reason to allow
    clippy::blanket_clippy_restriction_lints, // allow clippy::restriction
    clippy::implicit_return, // actually omitting the return keyword is idiomatic Rust code
    clippy::module_name_repetitions, // repeation of module name in a struct name is not big deal
    clippy::multiple_crate_versions, // multi-version dependency crates is not able to fix
    clippy::missing_errors_doc, // TODO: add error docs
    clippy::missing_panics_doc, // TODO: add panic docs
    clippy::panic_in_result_fn,
    clippy::shadow_same, // Not too much bad
    clippy::shadow_reuse, // Not too much bad
    clippy::exhaustive_enums,
    clippy::exhaustive_structs,
    clippy::indexing_slicing,
    clippy::separated_literal_suffix, // conflicts with clippy::unseparated_literal_suffix
    clippy::single_char_lifetime_names, // TODO: change lifetime names
)]
//! see `https://github.com/acl-dev/open-coroutine`

pub use open_coroutine_core::net::config::Config;
pub use open_coroutine_macros::*;

use open_coroutine_core::co_pool::task::UserTaskFunc;
use open_coroutine_core::common::constants::SLICE;
use open_coroutine_core::net::UserFunc;
use std::ffi::{c_int, c_long, c_uint, c_void};
use std::io::{Error, ErrorKind};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

extern "C" {
    fn open_coroutine_init(config: Config) -> c_int;

    fn open_coroutine_stop(secs: c_uint) -> c_int;

    fn task_crate(f: UserTaskFunc, param: usize) -> c_int;

    fn maybe_grow_stack(red_zone: usize, stack_size: usize, f: UserFunc, param: usize) -> c_long;
}

/// Init the open-coroutine.
pub fn init(config: Config) {
    assert_eq!(
        0,
        unsafe { open_coroutine_init(config) },
        "open-coroutine init failed !"
    );
}

/// Shutdown the open-coroutine.
pub fn shutdown() {
    assert_eq!(
        0,
        unsafe { open_coroutine_stop(30) },
        "open-coroutine shutdown failed !"
    );
}

/// Create a task.
#[macro_export]
macro_rules! task {
    ( $f: expr , $param:expr $(,)? ) => {
        $crate::task($f, $param)
    };
}

/// Create a task.
pub fn task<P: 'static, R: 'static, F: FnOnce(P) -> R>(f: F, param: P) -> c_int {
    extern "C" fn task_main<P: 'static, R: 'static, F: FnOnce(P) -> R>(input: usize) -> usize {
        unsafe {
            let ptr = &mut *((input as *mut c_void).cast::<(F, P)>());
            let data = std::ptr::read_unaligned(ptr);
            let result: &'static mut R = Box::leak(Box::new((data.0)(data.1)));
            std::ptr::from_mut::<R>(result).cast::<c_void>() as usize
        }
    }
    let inner = Box::leak(Box::new((f, param)));
    unsafe {
        task_crate(
            task_main::<P, R, F>,
            std::ptr::from_mut::<(F, P)>(inner).cast::<c_void>() as usize,
        )
    }
}

/// Grows the call stack if necessary.
#[macro_export]
macro_rules! maybe_grow {
    ($red_zone:expr, $stack_size:expr, $f:expr $(,)?) => {
        $crate::maybe_grow($red_zone, $stack_size, $f)
    };
    ($stack_size:literal, $f:expr $(,)?) => {
        $crate::maybe_grow(
            open_coroutine_core::common::default_red_zone(),
            $stack_size,
            $f,
        )
    };
    ($f:expr $(,)?) => {
        $crate::maybe_grow(
            open_coroutine_core::common::default_red_zone(),
            open_coroutine_core::common::constants::DEFAULT_STACK_SIZE,
            $f,
        )
    };
}

/// Create a coroutine.
pub fn maybe_grow<R: 'static, F: FnOnce() -> R>(
    red_zone: usize,
    stack_size: usize,
    f: F,
) -> std::io::Result<R> {
    extern "C" fn execute_on_stack<R: 'static, F: FnOnce() -> R>(input: usize) -> usize {
        unsafe {
            let ptr = &mut *((input as *mut c_void).cast::<F>());
            let data = std::ptr::read_unaligned(ptr);
            let result: &'static mut R = Box::leak(Box::new(data()));
            std::ptr::from_mut::<R>(result).cast::<c_void>() as usize
        }
    }
    let inner = Box::leak(Box::new(f));
    unsafe {
        let ptr = maybe_grow_stack(
            red_zone,
            stack_size,
            execute_on_stack::<R, F>,
            std::ptr::from_mut::<F>(inner).cast::<c_void>() as usize,
        );
        if ptr < 0 {
            return Err(Error::new(ErrorKind::InvalidInput, "grow stack failed"));
        }
        Ok(*Box::from_raw(
            usize::try_from(ptr).expect("overflow") as *mut R
        ))
    }
}

/// Opens a TCP connection to a remote host.
///
/// `addr` is an address of the remote host. Anything which implements
/// [`ToSocketAddrs`] trait can be supplied for the address; see this trait
/// documentation for concrete examples.
///
/// If `addr` yields multiple addresses, `connect` will be attempted with
/// each of the addresses until a connection is successful. If none of
/// the addresses result in a successful connection, the error returned from
/// the last connection attempt (the last address) is returned.
///
/// # Examples
///
/// Open a TCP connection to `127.0.0.1:8080`:
///
/// ```no_run
/// if let Ok(stream) = open_coroutine::connect_timeout("127.0.0.1:8080", std::time::Duration::from_secs(3)) {
///     println!("Connected to the server!");
/// } else {
///     println!("Couldn't connect to server...");
/// }
/// ```
///
/// Open a TCP connection to `127.0.0.1:8080`. If the connection fails, open
/// a TCP connection to `127.0.0.1:8081`:
///
/// ```no_run
/// use std::net::SocketAddr;
///
/// let addrs = [
///     SocketAddr::from(([127, 0, 0, 1], 8080)),
///     SocketAddr::from(([127, 0, 0, 1], 8081)),
/// ];
/// if let Ok(stream) = open_coroutine::connect_timeout(&addrs[..], std::time::Duration::from_secs(3)) {
///     println!("Connected to the server!");
/// } else {
///     println!("Couldn't connect to server...");
/// }
/// ```
pub fn connect_timeout<A: ToSocketAddrs>(addr: A, timeout: Duration) -> std::io::Result<TcpStream> {
    let timeout_time = open_coroutine_core::common::get_timeout_time(timeout);
    let mut last_err = None;
    for addr in addr.to_socket_addrs()? {
        loop {
            let left_time = timeout_time.saturating_sub(open_coroutine_core::common::now());
            if 0 == left_time {
                break;
            }
            match TcpStream::connect_timeout(&addr, Duration::from_nanos(left_time).min(SLICE)) {
                Ok(l) => return Ok(l),
                Err(e) => last_err = Some(e),
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        Error::new(
            ErrorKind::InvalidInput,
            "could not resolve to any addresses",
        )
    }))
}

#[cfg(test)]
mod tests {
    use crate::{init, shutdown};
    use open_coroutine_core::net::config::Config;

    #[test]
    fn test_link() {
        init(Config::single());
        #[cfg(not(windows))]
        {
            _ = task!(
                move |_| {
                    fn recurse(i: u32, p: &mut [u8; 10240]) {
                        maybe_grow!(|| {
                            // Ensure the stack allocation isn't optimized away.
                            unsafe { _ = std::ptr::read_volatile(&p) };
                            if i > 0 {
                                recurse(i - 1, &mut [0; 10240]);
                            }
                        })
                        .expect("allocate stack failed")
                    }
                    // Use ~500KB of stack.
                    recurse(50, &mut [0; 10240]);
                    // Use ~500KB of stack.
                    recurse(50, &mut [0; 10240]);
                },
                (),
            );
        }
        shutdown();
    }
}
