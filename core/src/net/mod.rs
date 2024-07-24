mod selector;

#[allow(dead_code)]
#[cfg(all(target_os = "linux", feature = "io_uring"))]
mod operator;

mod event_loop;

mod facade;
