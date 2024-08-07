use std::error::Error;
use std::ffi::c_void;
use windows_sys::Win32::Foundation::BOOL;
use windows_sys::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};

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
        BOOL::from(attach().is_ok())
    } else if call_reason == DLL_PROCESS_DETACH {
        BOOL::from(detach().is_ok())
    } else {
        BOOL::TRUE
    }
}

/// Called when the DLL is attached to the process.
unsafe fn attach() -> Result<(), Box<dyn Error>> {
    crochet::enable!(Sleep_hook)?;
    Ok(())
}

/// Called when the DLL is detached to the process.
unsafe fn detach() -> Result<(), Box<dyn Error>> {
    crochet::disable!(Sleep_hook)?;
    Ok(())
}

#[crochet::hook(compile_check, "kernel32.dll", "Sleep")]
fn Sleep_hook(dw_milliseconds: u32) {
    open_coroutine_core::syscall::Sleep(
        Some(|dw_milliseconds| call_original!(dw_milliseconds)),
        dw_milliseconds,
    );
}
