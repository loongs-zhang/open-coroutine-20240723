mod selector;

#[cfg(all(target_os = "linux", feature = "io_uring"))]
mod operator;

mod event_loop;
