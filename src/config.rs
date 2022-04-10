//! # Configuration
//!
//! Utilities to configure the text editor.

use std::time::Duration;

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
    pub show_line_num: bool
}

impl Default for Config {
    /// Default configuration.
    fn default() -> Self {
        Self { tab_stop: 4, quit_times: 2, message_dur: Duration::new(3, 0), show_line_num: true }
    }
}
