use crate::common::CondvarBlocker;
use polling::{Event, PollMode};
use std::ffi::c_int;
use std::num::NonZeroUsize;
use std::ops::{Deref, DerefMut};
use std::os::fd::{FromRawFd, OwnedFd};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub(crate) struct Events(polling::Events);

impl Events {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self(polling::Events::with_capacity(
            NonZeroUsize::new(capacity).expect("capacity must be greater than 0"),
        ))
    }
}

impl Deref for Events {
    type Target = polling::Events;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Events {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl super::Interest for Event {
    fn read(token: usize) -> Self {
        Event::readable(token)
    }

    fn write(token: usize) -> Self {
        Event::writable(token)
    }

    fn read_and_write(token: usize) -> Self {
        Event::all(token)
    }
}

impl super::Event for Event {
    fn get_token(&self) -> usize {
        self.key
    }

    fn readable(&self) -> bool {
        self.readable
    }

    fn writable(&self) -> bool {
        self.writable
    }
}

#[derive(Debug)]
pub(crate) struct Poller {
    waiting: AtomicBool,
    blocker: CondvarBlocker,
    inner: polling::Poller,
}

impl Poller {
    pub(crate) fn new() -> std::io::Result<Self> {
        Ok(Self {
            waiting: AtomicBool::new(false),
            blocker: CondvarBlocker::default(),
            inner: polling::Poller::new()?,
        })
    }
}

impl Deref for Poller {
    type Target = polling::Poller;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Poller {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl super::Selector<Event, Event> for Poller {
    fn waiting(&self) -> &AtomicBool {
        &self.waiting
    }

    fn blocker(&self) -> &CondvarBlocker {
        &self.blocker
    }

    fn do_select(&self, events: &mut Events, timeout: Option<Duration>) -> std::io::Result<()> {
        self.wait(events, timeout).map(|_| ())
    }

    fn do_register(&self, fd: c_int, _: usize, interests: Event) -> std::io::Result<()> {
        cfg_if::cfg_if! {
            if #[cfg(windows)] {
                let source = std::os::windows::io::RawSocket::from(fd as u32);
            } else {
                let source = fd;
            }
        }
        unsafe {
            self.add_with_mode(
                source,
                interests,
                if self.supports_edge() {
                    PollMode::Edge
                } else {
                    PollMode::Level
                },
            )
        }
    }

    fn do_reregister(&self, fd: c_int, _: usize, interests: Event) -> std::io::Result<()> {
        unsafe {
            self.modify_with_mode(
                OwnedFd::from_raw_fd(fd),
                interests,
                if self.supports_edge() {
                    PollMode::Edge
                } else {
                    PollMode::Level
                },
            )
        }
    }

    fn do_deregister(&self, fd: c_int, _: usize) -> std::io::Result<()> {
        self.delete(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}
