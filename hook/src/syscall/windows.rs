use std::error::Error;
use std::ffi::{c_void, CString};
use windows_sys::Win32::Foundation::BOOL;
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows_sys::Win32::System::SystemServices::DLL_PROCESS_ATTACH;

// check https://www.rustwiki.org.cn/en/reference/introduction.html for help information
#[allow(unused_macros)]
macro_rules! impl_hook {
    ( $module_name: expr, $field_name: ident, $syscall: ident($($arg: ident : $arg_type: ty),*) -> $result: ty ) => {
        let syscall_name = open_coroutine_core::common::constants::Syscall::$syscall.into();
        let address = get_module_symbol_address($module_name, syscall_name)
            .unwrap_or_else(|| panic!("could not find {syscall_name} address"));
        let target: unsafe extern "system" fn($($arg_type),*) = std::mem::transmute(address);
        retour::static_detour! {
            static $field_name: unsafe extern "system" fn($($arg_type),*);
        }
        #[allow(non_snake_case)]
        fn $syscall($($arg: $arg_type),*) {
            open_coroutine_core::syscall::$syscall(Some(&$field_name), $($arg),*);
        }
        $field_name.initialize(target, $syscall)?.enable()?;
    }
}

#[no_mangle]
#[allow(non_snake_case, clippy::missing_safety_doc)]
pub unsafe extern "system" fn DllMain(
    _module: *mut c_void,
    call_reason: u32,
    _reserved: *mut c_void,
) -> BOOL {
    if call_reason == DLL_PROCESS_ATTACH {
        // A console may be useful for printing to 'stdout'
        // winapi::um::consoleapi::AllocConsole();

        // Preferably a thread should be created here instead, since as few
        // operations as possible should be performed within `DllMain`.
        BOOL::from(main().is_ok())
    } else {
        1
    }
}

/// Called when the DLL is attached to the process.
unsafe fn main() -> Result<(), Box<dyn Error>> {
    impl_hook!("kernel32.dll", SLEEP, Sleep(dw_milliseconds: u32) -> ());
    Ok(())
}

/// Returns a module symbol's absolute address.
fn get_module_symbol_address(module: &str, symbol: &str) -> Option<usize> {
    let module = module
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<u16>>();
    let symbol = CString::new(symbol).unwrap();
    unsafe {
        let handle = GetModuleHandleW(module.as_ptr());
        GetProcAddress(handle, symbol.as_ptr().cast()).map(|n| n as usize)
    }
}
