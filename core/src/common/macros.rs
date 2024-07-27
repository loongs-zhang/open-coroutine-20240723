/// Fast impl `Display` trait for `Debug` types.
/// Check <https://www.rustwiki.org.cn/en/reference/introduction.html> for help information.
#[allow(unused_macros)]
#[macro_export]
macro_rules! impl_display_by_debug {
    ($struct_name:ident$(<$($generic1:tt $( : $trait_tt1: tt $( + $trait_tt2: tt)*)?),+>)?
        $(where $(
            $generic2:tt $( : $trait_tt3: tt $( + $trait_tt4: tt)*)?
        ),+)?
    ) => {
        impl$(<$($generic1 $( : $trait_tt1 $( + $trait_tt2)*)?),+>)? std::fmt::Display
            for $struct_name$(<$($generic1),+>)?
        where
            $($($generic2 $( : $trait_tt3 $( + $trait_tt4)*)?),+,)?
            $struct_name$(<$($generic1),+>)?: std::fmt::Debug,
        {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Debug::fmt(self, f)
            }
        }
    };
}

/// Fast impl `Current` for a type.
/// This crate use `std` cause `#![no_std]` not support `thread_local!`.
/// Check <https://www.rustwiki.org.cn/en/reference/introduction.html> for help information.
#[allow(unused_macros)]
#[macro_export]
macro_rules! impl_current_for {
    (
        $name:ident,
        $struct_name:ident$(<$($generic1:tt $( : $trait_tt1: tt $( + $trait_tt2: tt)*)?),+>)?
        $(where $(
            $generic2:tt $( : $trait_tt3: tt $( + $trait_tt4: tt)*)?
        ),+)?
    ) => {
        thread_local! {
            static $name: std::cell::RefCell<std::collections::VecDeque<*const std::ffi::c_void>> = const { std::cell::RefCell::new(std::collections::VecDeque::new()) };
        }

        impl$(<$($generic1 $( : $trait_tt1 $( + $trait_tt2)*)?),+>)? $crate::common::traits::Current for $struct_name$(<$($generic1),+>)?
            $(where $($generic2 $( : $trait_tt3 $( + $trait_tt4)*)?),+)?
        {
            fn init_current(current: &Self) {
                $name.with(|s| {
                    s.borrow_mut()
                        .push_front(core::ptr::from_ref(current).cast::<std::ffi::c_void>());
                });
            }

            fn current<'current>() -> Option<&'current Self> {
                $name.with(|s| {
                    s.borrow()
                        .front()
                        .map(|ptr| unsafe { &*(*ptr).cast::<Self>() })
                })
            }

            fn clean_current() {
                $name.with(|s| _ = s.borrow_mut().pop_front());
            }
        }
    };
}
