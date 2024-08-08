pub use Sleep::Sleep;

macro_rules! impl_facade {
    ( $struct_name:ident, $trait_name: ident, $syscall: ident($($arg: ident : $arg_type: ty),*) -> $result: ty ) => {
        #[repr(C)]
        #[derive(Debug, Default)]
        struct $struct_name<I: $trait_name> {
            inner: I,
        }

        impl<I: $trait_name> $trait_name for $struct_name<I> {
            extern "system" fn $syscall(
                &self,
                fn_ptr: Option<&retour::StaticDetour<unsafe extern "system" fn($($arg_type),*) -> $result>>,
                $($arg: $arg_type),*
            ) -> $result {
                // use $crate::constants::{Syscall, SyscallState};
                // use $crate::scheduler::SchedulableCoroutine;
                //
                let syscall = $crate::common::constants::Syscall::$syscall;
                $crate::info!("enter syscall {}", syscall);
                // if let Some(co) = SchedulableCoroutine::current() {
                //     let new_state = SyscallState::Executing;
                //     if co.syscall((), syscall, new_state).is_err() {
                //         $crate::error!("{} change to syscall {} {} failed !",
                //             co.get_name(), syscall, new_state);
                //     }
                // }
                let r = self.inner.$syscall(fn_ptr, $($arg, )*);
                // if let Some(co) = SchedulableCoroutine::current() {
                //     if co.running().is_err() {
                //         $crate::error!("{} change to running state failed !", co.get_name());
                //     }
                // }
                $crate::info!("exit syscall {}", syscall);
                r
            }
        }
    }
}

#[allow(unused_macros)]
macro_rules! impl_raw {
    ( $struct_name: ident, $trait_name: ident, $($mod_name: ident)::*, $syscall: ident($($arg: ident : $arg_type: ty),*) -> $result: ty ) => {
        #[repr(C)]
        #[derive(Debug, Copy, Clone, Default)]
        struct $struct_name {}

        impl $trait_name for $struct_name {
            extern "system" fn $syscall(
                &self,
                fn_ptr: Option<&retour::StaticDetour<unsafe extern "system" fn($($arg_type),*)> -> $result>,
                $($arg: $arg_type),*
            ) -> $result {
                unsafe {
                    if let Some(f) = fn_ptr {
                        f.call($($arg),*)
                    } else {
                        $($mod_name)::*::$syscall($($arg),*)
                    }
                }
            }
        }
    }
}

mod Sleep;
