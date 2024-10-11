use once_cell::sync::Lazy;
use std::ffi::{c_longlong, c_uint, c_void};
use windows_sys::Win32::Foundation::{HANDLE, NTSTATUS, STATUS_SUCCESS};
use windows_sys::Win32::System::IO::{IO_STATUS_BLOCK, PIO_APC_ROUTINE};

#[must_use]
pub extern "system" fn NtReadFile(
    fn_ptr: Option<
        &extern "system" fn(
            HANDLE,
            HANDLE,
            PIO_APC_ROUTINE,
            *const c_void,
            *mut IO_STATUS_BLOCK,
            *mut c_void,
            c_uint,
            *const c_longlong,
            *const c_uint,
        ) -> NTSTATUS,
    >,
    filehandle: HANDLE,
    event: HANDLE,
    apcroutine: PIO_APC_ROUTINE,
    apccontext: *const c_void,
    iostatusblock: *mut IO_STATUS_BLOCK,
    buffer: *mut c_void,
    length: c_uint,
    byteoffset: *const c_longlong,
    key: *const c_uint,
) -> NTSTATUS {
    static CHAIN: Lazy<NtReadFileSyscallFacade<NioNtReadFileSyscall<RawNtReadFileSyscall>>> =
        Lazy::new(Default::default);
    CHAIN.NtReadFile(
        fn_ptr,
        filehandle,
        event,
        apcroutine,
        apccontext,
        iostatusblock,
        buffer,
        length,
        byteoffset,
        key,
    )
}

trait NtReadFileSyscall {
    extern "system" fn NtReadFile(
        &self,
        fn_ptr: Option<
            &extern "system" fn(
                HANDLE,
                HANDLE,
                PIO_APC_ROUTINE,
                *const c_void,
                *mut IO_STATUS_BLOCK,
                *mut c_void,
                c_uint,
                *const c_longlong,
                *const c_uint,
            ) -> NTSTATUS,
        >,
        filehandle: HANDLE,
        event: HANDLE,
        apcroutine: PIO_APC_ROUTINE,
        apccontext: *const c_void,
        iostatusblock: *mut IO_STATUS_BLOCK,
        buffer: *mut c_void,
        length: c_uint,
        byteoffset: *const c_longlong,
        key: *const c_uint,
    ) -> NTSTATUS;
}

impl_facade!(NtReadFileSyscallFacade, NtReadFileSyscall,
    NtReadFile(
        filehandle : HANDLE,
        event : HANDLE,
        apcroutine : PIO_APC_ROUTINE,
        apccontext : *const c_void,
        iostatusblock : *mut IO_STATUS_BLOCK,
        buffer : *mut c_void,
        length : c_uint,
        byteoffset : *const c_longlong,
        key : *const c_uint
    ) -> NTSTATUS
);

#[repr(C)]
#[derive(Debug, Default)]
struct NioNtReadFileSyscall<I: NtReadFileSyscall> {
    inner: I,
}

impl<I: NtReadFileSyscall> NtReadFileSyscall for NioNtReadFileSyscall<I> {
    extern "system" fn NtReadFile(
        &self,
        fn_ptr: Option<
            &extern "system" fn(
                HANDLE,
                HANDLE,
                PIO_APC_ROUTINE,
                *const c_void,
                *mut IO_STATUS_BLOCK,
                *mut c_void,
                c_uint,
                *const c_longlong,
                *const c_uint,
            ) -> NTSTATUS,
        >,
        filehandle: HANDLE,
        event: HANDLE,
        apcroutine: PIO_APC_ROUTINE,
        apccontext: *const c_void,
        iostatusblock: *mut IO_STATUS_BLOCK,
        buffer: *mut c_void,
        length: c_uint,
        byteoffset: *const c_longlong,
        key: *const c_uint,
    ) -> NTSTATUS {
        let mut received = 0usize;
        let mut r = STATUS_SUCCESS;
        while received < length as usize {
            r = self.inner.NtReadFile(
                fn_ptr,
                filehandle,
                event,
                apcroutine,
                apccontext,
                iostatusblock,
                (buffer as usize + received) as *mut c_void,
                length - received as c_uint,
                byteoffset,
                key,
            );
            if STATUS_SUCCESS == r {
                crate::syscall::common::reset_errno();
                received += unsafe { (*iostatusblock).Information };
                if received >= length as usize || r == 0 {
                    unsafe { (*iostatusblock).Information = received };
                    break;
                }
            }
            let error_kind = std::io::Error::last_os_error().kind();
            if error_kind == std::io::ErrorKind::WouldBlock {
                if crate::net::EventLoops::wait_read_event(
                    filehandle as _,
                    Some(crate::common::constants::SLICE),
                )
                .is_err()
                {
                    break;
                }
            } else if error_kind != std::io::ErrorKind::Interrupted {
                break;
            }
        }
        r
    }
}

impl_raw!(RawNtReadFileSyscall, NtReadFileSyscall, windows_sys::Wdk::Storage::FileSystem,
    NtReadFile(
        filehandle : HANDLE,
        event : HANDLE,
        apcroutine : PIO_APC_ROUTINE,
        apccontext : *const c_void,
        iostatusblock : *mut IO_STATUS_BLOCK,
        buffer : *mut c_void,
        length : c_uint,
        byteoffset : *const c_longlong,
        key : *const c_uint
    ) -> NTSTATUS
);
