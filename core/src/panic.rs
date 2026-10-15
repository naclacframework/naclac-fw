//! Panic support a program binary installs on the Solana target. Which of the
//! two applies depends on whether `std` is anywhere in the program's build:
//! with `std`, std's own panic handler runs and calls the program's
//! `custom_panic` hook; with no `std` at all, the program must define the
//! `#[panic_handler]` itself, and defining one alongside `std` fails to
//! compile. `#[program]` picks between them.

#[doc(hidden)]
pub use solana_define_syscall::definitions::{
    abort as __abort, sol_log_ as __log, sol_panic_ as __panic,
};

/// Defines the `custom_panic` hook std's panic handler calls on the Solana
/// target: logs the panic message when it is a plain string, then reports the
/// panic location and aborts. For a program whose build includes `std`.
/// Omitted when the program crate enables its own `custom-panic` feature.
#[doc(hidden)]
#[macro_export]
macro_rules! __naclac_panic_hook {
    () => {
        #[cfg(all(not(feature = "custom-panic"), target_os = "solana"))]
        #[no_mangle]
        fn custom_panic(info: &::core::panic::PanicInfo<'_>) {
            if let Some(message) = info.message().as_str() {
                unsafe { $crate::panic::__log(message.as_ptr(), message.len() as u64) };
            }
            if let Some(location) = info.location() {
                unsafe {
                    $crate::panic::__panic(
                        location.file().as_ptr(),
                        location.file().len() as u64,
                        location.line() as u64,
                        location.column() as u64,
                    )
                }
            }
        }
    };
}

/// Defines the `abort` symbol that compiler-generated abort paths call on a
/// static-syscalls Solana target, as the platform's `std` does when it is
/// linked. On other targets the loader resolves `abort` as a syscall, so
/// defining it there would make the call recurse; this then expands to nothing.
#[cfg(target_feature = "static-syscalls")]
#[doc(hidden)]
#[macro_export]
macro_rules! __naclac_abort_symbol {
    () => {
        #[no_mangle]
        extern "C" fn abort() -> ! {
            unsafe { $crate::panic::__abort() }
        }
    };
}

#[cfg(not(target_feature = "static-syscalls"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __naclac_abort_symbol {
    () => {};
}

/// Defines the program's `#[panic_handler]` on the Solana target: logs the
/// panic message when it is a plain string, then reports the panic location
/// and aborts. Also defines the `abort` symbol (`__naclac_abort_symbol!`). For
/// a program with no `std` anywhere in its build. Omitted when the program
/// crate enables its own `custom-panic` feature.
#[doc(hidden)]
#[macro_export]
macro_rules! __naclac_panic_handler {
    () => {
        #[cfg(all(not(feature = "custom-panic"), target_os = "solana"))]
        $crate::__naclac_abort_symbol!();

        #[cfg(all(not(feature = "custom-panic"), target_os = "solana"))]
        #[panic_handler]
        fn __naclac_panic(info: &::core::panic::PanicInfo<'_>) -> ! {
            if let Some(message) = info.message().as_str() {
                unsafe { $crate::panic::__log(message.as_ptr(), message.len() as u64) };
            }
            match info.location() {
                Some(location) => unsafe {
                    $crate::panic::__panic(
                        location.file().as_ptr(),
                        location.file().len() as u64,
                        location.line() as u64,
                        location.column() as u64,
                    )
                },
                None => unsafe { $crate::panic::__abort() },
            }
        }
    };
}
