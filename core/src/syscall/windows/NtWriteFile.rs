use once_cell::sync::Lazy;
use std::ffi::{c_longlong, c_uint, c_void};
use windows_sys::Win32::Foundation::{HANDLE, NTSTATUS, STATUS_SUCCESS};
use windows_sys::Win32::System::IO::{IO_STATUS_BLOCK, PIO_APC_ROUTINE};

#[must_use]
pub extern "system" fn NtWriteFile(
    fn_ptr: Option<
        &extern "system" fn(
            HANDLE,
            HANDLE,
            PIO_APC_ROUTINE,
            *const c_void,
            *mut IO_STATUS_BLOCK,
            *const c_void,
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
    buffer: *const c_void,
    length: c_uint,
    byteoffset: *const c_longlong,
    key: *const c_uint,
) -> NTSTATUS {
    static CHAIN: Lazy<NtWriteFileSyscallFacade<NioNtWriteFileSyscall<RawNtWriteFileSyscall>>> =
        Lazy::new(Default::default);
    CHAIN.NtWriteFile(
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

trait NtWriteFileSyscall {
    extern "system" fn NtWriteFile(
        &self,
        fn_ptr: Option<
            &extern "system" fn(
                HANDLE,
                HANDLE,
                PIO_APC_ROUTINE,
                *const c_void,
                *mut IO_STATUS_BLOCK,
                *const c_void,
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
        buffer: *const c_void,
        length: c_uint,
        byteoffset: *const c_longlong,
        key: *const c_uint,
    ) -> NTSTATUS;
}

impl_facade!(NtWriteFileSyscallFacade, NtWriteFileSyscall,
    NtWriteFile(
        filehandle: HANDLE,
        event: HANDLE,
        apcroutine: PIO_APC_ROUTINE,
        apccontext: *const c_void,
        iostatusblock: *mut IO_STATUS_BLOCK,
        buffer: *const c_void,
        length: c_uint,
        byteoffset: *const c_longlong,
        key: *const c_uint
    ) -> NTSTATUS
);

#[repr(C)]
#[derive(Debug, Default)]
struct NioNtWriteFileSyscall<I: NtWriteFileSyscall> {
    inner: I,
}

impl<I: NtWriteFileSyscall> NtWriteFileSyscall for NioNtWriteFileSyscall<I> {
    extern "system" fn NtWriteFile(
        &self,
        fn_ptr: Option<
            &extern "system" fn(
                HANDLE,
                HANDLE,
                PIO_APC_ROUTINE,
                *const c_void,
                *mut IO_STATUS_BLOCK,
                *const c_void,
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
        buffer: *const c_void,
        length: c_uint,
        byteoffset: *const c_longlong,
        key: *const c_uint,
    ) -> NTSTATUS {
        let mut sent = 0usize;
        let mut r = STATUS_SUCCESS;
        while sent < length as usize {
            r = self.inner.NtWriteFile(
                fn_ptr,
                filehandle,
                event,
                apcroutine,
                apccontext,
                iostatusblock,
                (buffer as usize + sent) as *const c_void,
                length - sent as c_uint,
                byteoffset,
                key,
            );
            if STATUS_SUCCESS == r {
                crate::syscall::common::reset_errno();
                sent += unsafe { (*iostatusblock).Information };
                if sent >= length as usize {
                    unsafe { (*iostatusblock).Information = sent };
                    break;
                }
            }
            let error_kind = std::io::Error::last_os_error().kind();
            if error_kind == std::io::ErrorKind::WouldBlock {
                if crate::net::EventLoops::wait_write_event(
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

impl_raw!(RawNtWriteFileSyscall, NtWriteFileSyscall, windows_sys::Wdk::Storage::FileSystem,
    NtWriteFile(
        filehandle: HANDLE,
        event: HANDLE,
        apcroutine: PIO_APC_ROUTINE,
        apccontext: *const c_void,
        iostatusblock: *mut IO_STATUS_BLOCK,
        buffer: *const c_void,
        length: c_uint,
        byteoffset: *const c_longlong,
        key: *const c_uint
    ) -> NTSTATUS
);
