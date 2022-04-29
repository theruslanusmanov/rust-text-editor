//! # sys (UNIX)
//!
//! UNIX-specific structs and functions. Will be imported as `sys` on UNIX systems.

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

// On UNIX systems, termios represents the terminal mode.
pub use libc::termios as TermMode;
use libc::{c_int, c_void, sigaction, sighandler_t, siginfo_t, winsize};
use libc::{SA_SIGINFO, STDIN_FILENO, STDOUT_FILENO, TCSADRAIN, TIOCGWINSZ, VMIN, VTIME};

pub use crate::xdg::*;
use crate::Error;

fn cerr(err: c_int) -> Result<(), Error> {
    match err {
        0..=c_int::MAX => Ok(()),
        _ => Err(std::io::Error::last_os_error().into()),
    }
}
