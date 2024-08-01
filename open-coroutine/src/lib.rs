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

use open_coroutine_core::common::constants::SLICE;
use std::ffi::{c_int, c_uint};
use std::io::{Error, ErrorKind};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

extern "C" {
    fn open_coroutine_init(config: Config) -> c_int;

    fn open_coroutine_stop(secs: c_uint) -> c_int;
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
    let mut left_time = timeout;
    let mut last_err = None;
    for addr in addr.to_socket_addrs()? {
        while left_time > Duration::ZERO {
            match TcpStream::connect_timeout(&addr, left_time.min(SLICE)) {
                Ok(l) => return Ok(l),
                Err(e) => last_err = Some(e),
            }
            left_time = left_time.saturating_sub(SLICE);
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
        shutdown();
    }
}
