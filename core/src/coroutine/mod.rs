use crate::common::constants::CoroutineState;
use crate::coroutine::listener::Listener;
use crate::coroutine::local::CoroutineLocal;
use crate::{impl_current_for, impl_display_by_debug, impl_for_named};
use std::fmt::{Debug, Formatter};
use std::ops::Deref;

/// Coroutine suspender abstraction and impl.
#[allow(dead_code)]
pub mod suspender;

/// Coroutine local abstraction.
pub mod local;

/// Coroutine listener abstraction and impl.
pub mod listener;

#[cfg(feature = "korosensei")]
pub use korosensei::Coroutine;
#[cfg(feature = "korosensei")]
mod korosensei;

/// Create a new coroutine.
#[macro_export]
macro_rules! co {
    ($f:expr, $size:literal $(,)?) => {
        $crate::coroutine::Coroutine::new(uuid::Uuid::new_v4().to_string(), $f, $size)
    };
    ($f:expr $(,)?) => {
        $crate::coroutine::Coroutine::new(
            uuid::Uuid::new_v4().to_string(),
            $f,
            $crate::common::constants::DEFAULT_STACK_SIZE,
        )
    };
    ($name:expr, $f:expr, $size:expr $(,)?) => {
        $crate::coroutine::Coroutine::new($name, $f, $size)
    };
    ($name:expr, $f:expr $(,)?) => {
        $crate::coroutine::Coroutine::new($name, $f, $crate::common::constants::DEFAULT_STACK_SIZE)
    };
}

/// Coroutine state abstraction and impl.
mod state;

impl<'c, Param, Yield, Return> Coroutine<'c, Param, Yield, Return> {
    /// Get the name of this object.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the current state of this `StateCoroutine`.
    pub fn state(&self) -> CoroutineState<Yield, Return>
    where
        Yield: Copy,
        Return: Copy,
    {
        self.state.get()
    }

    /// Add a listener to this coroutine.
    pub fn add_listener(&mut self, listener: impl Listener<Yield, Return> + 'c) {
        self.add_raw_listener(Box::leak(Box::new(listener)));
    }
}

impl<Yield, Return> Coroutine<'_, (), Yield, Return>
where
    Yield: Debug + Copy + Eq,
    Return: Debug + Copy + Eq,
{
    /// A simpler version of [`Coroutine::resume_with`].
    pub fn resume(&mut self) -> std::io::Result<CoroutineState<Yield, Return>> {
        self.resume_with(())
    }
}

impl<Param, Yield, Return> Coroutine<'_, Param, Yield, Return>
where
    Yield: Debug + Copy + Eq,
    Return: Debug + Copy + Eq,
{
    /// Resumes the execution of this coroutine.
    ///
    /// The argument will be passed into the coroutine as a resume argument.
    ///
    /// # Errors
    /// if current coroutine state is unexpected.
    pub fn resume_with(&mut self, arg: Param) -> std::io::Result<CoroutineState<Yield, Return>> {
        let current = self.state();
        if let CoroutineState::Complete(r) = current {
            return Ok(CoroutineState::Complete(r));
        }
        if let CoroutineState::Error(e) = current {
            return Ok(CoroutineState::Error(e));
        }
        Self::init_current(self);
        self.running()?;
        let r = self.raw_resume(arg);
        Self::clean_current();
        r
    }
}

impl<Param, Yield, Return> Debug for Coroutine<'_, Param, Yield, Return>
where
    Yield: Debug + Copy,
    Return: Debug + Copy,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Coroutine")
            .field("name", &self.name())
            .field("status", &self.state())
            .field("local", &self.local)
            .finish()
    }
}

impl<'c, Param, Yield, Return> Deref for Coroutine<'c, Param, Yield, Return> {
    type Target = CoroutineLocal<'c>;

    fn deref(&self) -> &Self::Target {
        &self.local
    }
}

impl_display_by_debug!(
    Coroutine<'c, Param, Yield, Return>
    where
        Yield: Debug + Copy,
        Return: Debug + Copy
);

impl_for_named!(Coroutine<'c, Param, Yield, Return>);

impl_current_for!(COROUTINE, Coroutine<'c, Param, Yield, Return>);
