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

/// Return the current window size as (rows, columns).
///
/// We use the `TIOCGWINSZ` ioctl to get window size. If it succeeds, a `Winsize` struct will be
/// populated.
/// This ioctl is described here: <http://man7.org/linux/man-pages/man4/tty_ioctl.4.html>
pub fn get_window_size() -> Result<(usize, usize), Error> {
    let mut maybe_ws = std::mem::MaybeUninit::<winsize>::uninit();
    cerr(unsafe { libc::ioctl(STDOUT_FILENO, TIOCGWINSZ, maybe_ws.as_mut_ptr()) })
        .map_or(None, |_| unsafe { Some(maybe_ws.assume_init()) })
        .filter(|ws| ws.ws_col != 0 && ws.ws_row != 0)
        .map_or(Err(Error::InvalidWindowSize), |ws| Ok((ws.ws_row as usize, ws.ws_col as usize)))
}

/// Stores whether the window size has changed since last call to `has_window_size_changed`.
static WSC: AtomicBool = AtomicBool::new(false);
