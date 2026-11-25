//! Debug-only Windows crash diagnostics: capture a symbolized backtrace when
//! the process dies of stack overflow (0xC00000FD), which the Rust runtime
//! otherwise reports without any backtrace.

#![cfg(target_os = "windows")]

use std::os::raw::{c_long, c_void};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn AddVectoredExceptionHandler(first: u32, handler: *const c_void) -> *mut c_void;
}

const STATUS_STACK_OVERFLOW: u32 = 0xC00000FD;

unsafe extern "system" fn handler(info: *mut c_void) -> c_long {
    // EXCEPTION_POINTERS { ExceptionRecord: *mut EXCEPTION_RECORD, ... }
    let record = unsafe { *(info as *const *const u32) };
    let code = unsafe { *record };
    if code != STATUS_STACK_OVERFLOW {
        return 0; // EXCEPTION_CONTINUE_SEARCH
    }
    // Resolve and write frames one at a time so partial output survives.
    if let Ok(mut file) = std::fs::File::create(r"C:\overflow-trace.txt") {
        use std::io::Write;
        let _ = writeln!(file, "stack overflow backtrace (innermost first):");
        let _ = file.flush();
        backtrace::trace(|frame| {
            let mut line = format!("{:?}", frame.ip());
            backtrace::resolve_frame(frame, |symbol| {
                if let Some(name) = symbol.name() {
                    line = format!("{:?} ! {:?}", frame.ip(), name);
                }
            });
            let _ = writeln!(file, "{line}");
            let _ = file.flush();
            // Keep enough context to see the repeating cycle without
            // exhausting what little stack remains.
            true
        });
        let _ = file.flush();
    }
    unsafe {
        std::process::exit(3);
    }
}

pub(crate) fn install() {
    unsafe {
        let _ = AddVectoredExceptionHandler(1, handler as *const c_void);
    }
}
