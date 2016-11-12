use std::sync::Condvar;
use std::sync::atomic::{AtomicBool, Ordering, ATOMIC_BOOL_INIT};

static mut CONDVAR: *const Condvar = 0 as *const Condvar;
static mut IS_CANCELLED: AtomicBool = ATOMIC_BOOL_INIT;

#[cfg(windows)]
mod detail {
    extern crate winapi;
    extern crate kernel32;

    use self::winapi::{DWORD, BOOL, TRUE};

    unsafe extern "system" fn ctrlc_handler(_: DWORD) -> BOOL {
        super::ctrlc_handler();
        TRUE
    }

    pub fn enable_ctrlc_handler() {
        unsafe {
            kernel32::SetConsoleCtrlHandler(Some(ctrlc_handler), TRUE);
        }
    }
}

#[cfg(unix)]
mod detail {
    extern crate nix;

    use self::nix::sys::signal;

    extern "C" fn ctrlc_handler(_: i32) {
        super::ctrlc_handler();
    }

    pub fn enable_ctrlc_handler() {
        let sig_action = signal::SigAction::new(signal::SigHandler::Handler(ctrlc_handler),
                                                signal::SaFlags::empty(),
                                                signal::SigSet::empty());
        unsafe {
            signal::sigaction(signal::SIGINT, &sig_action).unwrap();
        }
    }
}

fn ctrlc_handler() {
    unsafe {
        IS_CANCELLED.store(true, Ordering::Relaxed);
        (*CONDVAR).notify_all();
    }
}

pub fn set_ctrlc_handler(condvar: &Condvar) {
    unsafe {
        CONDVAR = condvar as *const Condvar;
    }
    detail::enable_ctrlc_handler();
}

pub fn cancelled() -> bool {
    unsafe { IS_CANCELLED.load(Ordering::Relaxed) }
}
