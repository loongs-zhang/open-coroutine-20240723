use std::ffi::c_void;
use std::io::{Error, ErrorKind};
use windows_sys::Win32::Foundation::{BOOL, TRUE};
use windows_sys::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};

// check https://www.rustwiki.org.cn/en/reference/introduction.html for help information
#[allow(unused_macros)]
macro_rules! impl_hook {
    ( $module_name: expr, $field_name: ident, $syscall: ident($($arg: ident : $arg_type: ty),*) -> $result: ty ) => {
        static $field_name: once_cell::sync::OnceCell<extern "system" fn($($arg_type),*) -> $result> =
            once_cell::sync::OnceCell::new();
        _ = $field_name.get_or_init(|| unsafe {
            let syscall: &str = open_coroutine_core::common::constants::Syscall::$syscall.into();
            let ptr = minhook::MinHook::create_hook_api($module_name, syscall, $syscall as _)
                .unwrap_or_else(|_| panic!("hook {syscall} failed !"));
            assert!(!ptr.is_null(), "syscall \"{syscall}\" not found !");
            std::mem::transmute(ptr)
        });
        #[allow(non_snake_case)]
        extern "system" fn $syscall($($arg: $arg_type),*) -> $result {
            open_coroutine_core::syscall::$syscall(
                Some($field_name.get().unwrap_or_else(|| {
                    panic!(
                        "hook {} failed !",
                        open_coroutine_core::common::constants::Syscall::$syscall
                    )
                })),
                $($arg),*
            );
        }
    }
}

#[no_mangle]
#[allow(non_snake_case, clippy::missing_safety_doc)]
pub unsafe extern "system" fn DllMain(
    _module: *mut c_void,
    call_reason: u32,
    _reserved: *mut c_void,
) -> BOOL {
    // Preferably a thread should be created here instead, since as few
    // operations as possible should be performed within `DllMain`.
    if call_reason == DLL_PROCESS_ATTACH {
        // Called when the DLL is attached to the process.
        BOOL::from(attach().is_ok())
    } else if call_reason == DLL_PROCESS_DETACH {
        // Called when the DLL is detached to the process.
        BOOL::from(minhook::MinHook::disable_all_hooks().is_ok())
    } else {
        TRUE
    }
}

unsafe fn attach() -> std::io::Result<()> {
    impl_hook!("kernel32.dll", SLEEP, Sleep(dw_milliseconds: u32) -> ());
    // Enable the hook
    minhook::MinHook::enable_all_hooks()
        .map_err(|_| Error::new(ErrorKind::Other, "init all hooks failed !"))
}
