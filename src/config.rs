//! # Configuration
//!
//! Utilities to configure the text editor.

use std::time::Duration;

use crate::Error;

#[derive(Debug, PartialEq)]
pub struct Config {
    /// The size of a tab. Must be > 0.
    pub tab_stop: usize,
    /// The number of confirmations needed before quitting, when changes have been made since the
    /// file was last changed.
    pub quit_times: usize,
    /// The duration for which messages are shown in the status bar.
    pub message_dur: Duration,
    /// Whether to display line numbers.
    pub show_line_num: bool,
}

impl Default for Config {
    /// Default configuration.
    fn default() -> Self {
        Self { tab_stop: 4, quit_times: 2, message_dur: Duration::new(3, 0), show_line_num: true }
    }
}

impl Config {
    /// Load the configuration, potentially overridden using `config.ini` files that can be located
    /// in the following directories:
    ///   - On Linux, macOS and other *nix systems:
    ///     - `/etc/rust-text-editor` (system-wide configuration).
    ///     - `$XDG_CONFIG_HOME/rust-text-editor` if environment variable `$XDG_CONFIG_HOME` is defined,
    ///         `$HOME/.config/rust-text-editor` otherwise (user-level configuration).
    ///   - On Windows:
    ///     - `%APPDATA%\rust-text-editor`
    ///
    /// # Errors
    ///
    /// Will return `Err` if one of the configuration file cannot be parsed properly.
    pub fn load() -> Result<Self, Error> {
        !todo!()
    }
}
