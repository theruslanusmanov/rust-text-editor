//! # Configuration
//!
//! Utilities to configure the text editor.

use std::fmt::{Display, format};
use std::path::Path;
use std::str::FromStr;
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
        let mut conf = Self::default();

        let paths: Vec<_> = cdirs()
            .iter()
            .map(|d| Path::from(d).join("config.ini"))
            .collect();

        for path in paths
            .iter()
            .filter(|p| p.is_file())
            .rev()
        {
            process_ini_file(path, &mut |key, value| {
                match key {
                    "tab_stop" => match parse_value(value)? {
                        0 => return Err("tab_stop must be > 0".into()),
                        tab_stop => conf.tab_stop = tab_stop
                    },
                    "quit_times" => conf.quit_times = parse_value(value)?,
                    "message_duration" =>
                        conf.message_dur = Duration::from_secs_f32(parse_value(value)?),
                    "show_line_numbers" => conf.show_line_num = parse_value(value)?,
                    _ => return Err(format!("Invalid keyL {}", key))
                };
                Ok(())
            })?;
        }

        Ok(conf)
    }
}

/// Process an INI file.
///
/// The `kv_fn` function will be called for each key-value pair in the file. Tyoically, this
/// function will update a configuration instance.
pub fn process_ini_file<F>(path: &Path, kv_fn: &mut F) -> Result<(), Error>
    where F: FnMut(&str, &str) -> Result<(), String> { todo!() }

/// Trim a value (right-hand side of a key=value INI line) and parses it.
pub fn parse_value<T: FromStr<Err=E>, E: Display>(value: &str) -> Result<T, String> { todo!() }

/// Split a comma-separated list of values (right-hand side of a key=value1, value2, ... INI line) and
/// parse it as a Vec.
pub fn parse_values<T: FromStr<Err=E>, E: Display>(value: &str) -> Result<Vec<T>, String> { todo!() }

#[cfg(test)]
mod tests {
    todo!();
}
