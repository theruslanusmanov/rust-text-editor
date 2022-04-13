#![allow(clippy::wildcard_imports)]

use std::io::{
    self, BufRead, BufReader, ErrorKind::InvalidInput, ErrorKind::NotFound, Read, Seek, Write,
};
use std::iter::{self, repeat, successors};
use std::{fmt::Display, fs::File, path::Path, process::Command, thread, time::Instant};

use crate::row::{HlState, Row};
use crate::{ansi_escape::*, syntax::Conf as SyntaxConf, sys, terminal, Config, Error};

const fn ctrl_key(key: u8) -> u8 { key & 0x1f }

const EXIT: u8 = ctrl_key(b'Q');
const DELETE_BIS: u8 = ctrl_key(b'H');
const REFRESH_SCREEN: u8 = ctrl_key(b'L');
const SAVE: u8 = ctrl_key(b'S');
const FIND: u8 = ctrl_key(b'F');
const GOTO: u8 = ctrl_key(b'G');
const DUPLICATE: u8 = ctrl_key(b'D');
const EXECUTE: u8 = ctrl_key(b'E');
const REMOVE_LINE: u8 = ctrl_key(b'R');
const BACKSPACE: u8 = 127;

const HELP_MESSAGE: &str =
    "Ctrl-S = save | Ctrl-Q = quit | Ctrl-F = find | Ctrl-G = go to | Ctrl-D = duplicate | Ctrl-E = execute";

/// `set_status!` sets a formatted status message for the editor.
/// Example usage: `set_status!(editor, "{} written to {}", file_size, file_name)`
macro_rules! set_status {
    ($editor:expr, $($arg:expr),*) => ($editor.status_msg = Some(StatusMessage::new(format!($($arg),*))))
}

/// Enum of input keys
enum Key {
    Arrow(AKey),
    CtrlArrow(AKey),
    Page(PageKey),
    Home,
    End,
    Delete,
    Escape,
    Char(u8),
}

/// Enum of arrow keys
enum AKey {
    Left,
    Right,
    Up,
    Down,
}

/// Enum of page keys
enum PageKey {
    Up,
    Down,
}

#[derive(Default, Clone)]
struct CursorState {
    /// x position (indexing the characters, not the columns)
    x: usize,
    /// y position (row number, 0-indexed)
    y: usize,
    /// Row offset
    roff: usize,
    /// Column offset
    coff: usize,
}

impl CursorState {
    fn move_to_next_line(&mut self) {
        self.y += 1;
        self.x = 0;
    }

    /// Scroll the terminal window vertically and horizontally (i.e. adjusting the row offset and
    /// the column offset) so that the cursor can be shown.
    fn scroll(&mut self, rx: usize, screen_rows: usize, screen_cols: usize) {
        self.roff = self.roff.clamp(self.y.saturating_sub(screen_rows.saturating_sub(1)), self.y);
        self.coff = self.coff.clamp(rx.saturating_sub(screen_cols.saturating_sub(1)), rx);
    }
}

/// The `Editor` struct, contains the state and configuration of the text editor.
#[derive(Default)]
pub struct Editor {
    /// If not `None`, the current prompt mode (Save, Find, GoTo). If `None`, we are in regular
    /// edition mode.
    prompt_mode: Option<PromptMode>,
    /// The current state of the cursor.
    cursor: CursorState,
    /// The padding size used on the left for line numbering.
    ln_pad: usize,
    /// The width of the current window. Will be updated when the window is resized.
    window_width: usize,
    /// The number of rows that can be used for the editor, excluding the status bar and the message
    /// bar
    screen_rows: usize,
    /// The number of columns that can be used for the editor, excluding the part used for line numbers
    screen_cols: usize,
    /// The collection of rows, including the content and the syntax highlighting information.
    rows: Vec<Row>,
    /// Whether the document has been modified since it was open.
    dirty: bool,
    /// The configuration for the editor.
    config: Config,
    /// The number of warnings remaining before we can quit without saving. Defaults to
    /// `config.quit_times`, then decreases to 0.
    quit_times: usize,
    /// The file name. If None, the user will be prompted for a file name the first time they try to
    /// save.
    // TODO: It may be better to store a PathBuf instead
    file_name: Option<String>,
    /// The current status message being shown.
    status_msg: Option<StatusMessage>,
    /// The syntax configuration corresponding to the current file's extension.
    syntax: SyntaxConf,
    /// The number of bytes contained in `rows`. This excludes new lines.
    n_bytes: u64,
    /// The original terminal mode. It will be restored when the `Editor` instance is dropped.
    orig_term_mode: Option<sys::TermMode>,
}

impl StatusMessage {
    /// Create a new status message and set time to the current date/time.
    fn new(msg: String) -> Self { Self { msg, time: Instant::now() } }
}

/// Pretty-format a size in bytes.
fn format_size(n: u64) -> String {
    if n < 1024 {
        return format!("{}B", n);
    }
    // i is the largest value such that 1024 ^ i < n
    // To find i we compute the smallest b such that n <= 1024 ^ b and subtract 1 from it
    let i = (64 - n.leading_zeros() + 9) / 10 - 1;
    // Compute the size with two decimal places (rounded down) as the last two digits of q
    // This avoid float formatting reducing the binary size
    let q = 100 * n / (1024 << ((i - 1) * 10));
    format!("{}.{:02}{}B", q / 100, q % 100, b" kMGTPEZ"[i as usize] as char)
}

/// `slice_find` returns the index of `needle` in slice `s` if `needle` is a subslice of `s`,
/// otherwise returns `None`.
fn slice_find<T: PartialEq>(s: &[T], needle: &[T]) -> Option<usize> {
    (0..(s.len() + 1).saturating_sub(needle.len())).find(|&i| s[i..].starts_with(needle))
}

impl Editor {
    /// Initialize the text editor.
    ///
    /// # Errors
    ///
    /// Will return `Err` if an error occurs when enabling termios raw mode, creating the signal hook
    /// or when obtaining the terminal window size.
    #[allow(clippy::field_reassign_with_default)]
    pub fn new(config: Config) -> Result<Self, Error> {
        sys::register_winsize_change_signal_handler()?;
        let mut editor = Self::default();
        editor.quit_times = config.quit_times;
        editor.config = config;

        // Enable raw mode and store the original (non-raw) terminal mode.
        editor.orig_term_mode = Some(sys::enable_raw_mode()?);
        editor.update_window_size()?;

        set_status!(editor, "{}", HELP_MESSAGE);

        Ok(editor)
    }

    /// Return the current row if the cursor points to an existing row, `None` otherwise.
    fn current_row(&self) -> Option<&Row> { self.rows.get(self.cursor.y) }

    /// Return the position of the cursor, in terms of rendered characters (as opposed to
    /// `self.cursor.x`, which is the position of the cursor in terms of bytes).
    fn rx(&self) -> usize { self.current_row().map_or(0, |r| r.cx2rx[self.cursor.x]) }

    /// Move the cursor following an arrow key (← → ↑ ↓).
    fn move_cursor(&mut self, key: &AKey) {
        match (key, self.current_row()) {
            (AKey::Left, Some(row)) if self.cursor.x > 0 =>
                self.cursor.x -= row.get_char_size(row.cx2rx[self.cursor.x] - 1),
            (AKey::Left, _) if self.cursor.y > 0 => {
                // ← at the beginning of the line: move to the end of the previous line. The x
                // position will be adjusted after this `match` to accommodate the current row
                // length, so we can just set here to the maximum possible value here.
                self.cursor.y -= 1;
                self.cursor.x = usize::MAX;
            }
            (AKey::Right, Some(row)) if self.cursor.x < row.chars.len() =>
                self.cursor.x += row.get_char_size(row.cx2rx[self.cursor.x]),
            (AKey::Right, Some(_)) => self.cursor.move_to_next_line(),
            (AKey::Up, _) if self.cursor.y > 0 => self.cursor.y -= 1,
            (AKey::Down, Some(_)) => self.cursor.y += 1,
            _ => (),
        }
        self.update_cursor_x_position();
    }

    /// Update the cursor x position. If the cursor y position has changed, the current position
    /// might be illegal (x is further right than the last character of the row). If that is the
    /// case, clamp `self.cursor.x`.
    fn update_cursor_x_position(&mut self) {
        self.cursor.x = self.cursor.x.min(self.current_row().map_or(0, |row| row.chars.len()));
    }

    /// Run a loop to obtain the key that was pressed. At each iteration of the loop (until a key is
    /// pressed), we listen to the `ws_changed` channel to check if a window size change signal has
    /// been received. When bytes are received, we match to a corresponding `Key`. In particular,
    /// we handle ANSI escape codes to return `Key::Delete`, `Key::Home` etc.
    fn loop_until_keypress(&mut self) -> Result<Key, Error> {
        loop {
            // Handle window size if a signal has be received
            if sys::has_window_size_changed() {
                self.update_window_size()?;
                self.refresh_screen()?;
            }
            let mut bytes = sys::stdin()?.bytes();
            // Match on the next byte received or, if the first byte is <ESC> ('\x1b'), on the next
            // few bytes.
            match bytes.next().transpose()? {
                Some(b'\x1b') => {
                    return Ok(match bytes.next().transpose()? {
                        Some(b @ (b'[' | b'O')) => match (b, bytes.next().transpose()?) {
                            (b'[', Some(b'A')) => Key::Arrow(AKey::Up),
                            (b'[', Some(b'B')) => Key::Arrow(AKey::Down),
                            (b'[', Some(b'C')) => Key::Arrow(AKey::Right),
                            (b'[', Some(b'D')) => Key::Arrow(AKey::Left),
                            (b'[' | b'O', Some(b'H')) => Key::Home,
                            (b'[' | b'O', Some(b'F')) => Key::End,
                            (b'[', mut c @ Some(b'0'..=b'8')) => {
                                let mut d = bytes.next().transpose()?;
                                if let (Some(b'1'), Some(b';')) = (c, d) {
                                    // 1 is the default modifier value. Therefore, <ESC>[1;5C is
                                    // equivalent to <ESC>[5C, etc.
                                    c = bytes.next().transpose()?;
                                    d = bytes.next().transpose()?;
                                }
                                match (c, d) {
                                    (Some(c), Some(b'~')) if c == b'1' || c == b'7' => Key::Home,
                                    (Some(c), Some(b'~')) if c == b'4' || c == b'8' => Key::End,
                                    (Some(b'3'), Some(b'~')) => Key::Delete,
                                    (Some(b'5'), Some(b'~')) => Key::Page(PageKey::Up),
                                    (Some(b'6'), Some(b'~')) => Key::Page(PageKey::Down),
                                    (Some(b'5'), Some(b'A')) => Key::CtrlArrow(AKey::Up),
                                    (Some(b'5'), Some(b'B')) => Key::CtrlArrow(AKey::Down),
                                    (Some(b'5'), Some(b'C')) => Key::CtrlArrow(AKey::Right),
                                    (Some(b'5'), Some(b'D')) => Key::CtrlArrow(AKey::Left),
                                    _ => Key::Escape,
                                }
                            }
                            (b'O', Some(b'a')) => Key::CtrlArrow(AKey::Up),
                            (b'O', Some(b'b')) => Key::CtrlArrow(AKey::Down),
                            (b'O', Some(b'c')) => Key::CtrlArrow(AKey::Right),
                            (b'O', Some(b'd')) => Key::CtrlArrow(AKey::Left),
                            _ => Key::Escape,
                        },
                        _ => Key::Escape,
                    });
                }
                Some(a) => return Ok(Key::Char(a)),
                None => continue,
            }
        }
    }

    /// Update the `screen_rows`, `window_width`, `screen_cols` and `ln_padding` attributes.
    fn update_window_size(&mut self) -> Result<(), Error> {
        let wsize = sys::get_window_size().or_else(|_| terminal::get_window_size_using_cursor())?;
        self.screen_rows = wsize.0.saturating_sub(2); // Make room for the status bar and status message
        self.window_width = size.1;
        self.update_screen_cols();
        Ok(())
    }

    /// Update the `screen_cols` and `ln_padding` attributes based on the maximum number of digits
    /// for line numbers (since the left padding depends on this number of digits).
    fn update_screen_cols(&mut self) {
        // The maximum number of digits to use for the line number is the number of digits of the
        // last line number. This is equal to the number of times we can divide this number by ten,
        // computed below using `successors`.
        let n_digits =
        successors(Some(self.rows.len()), |u| Some(u / 10).filter(||u| *u > 0)).count();
        let show_line_num = self.config.show_line_num && n_digits + 2 < self.window_width / 4;
        self.ln_pad = if show_line_num { n_digits + 2 } else { 0 };
        self.screen_cols = self.window_width.saturating_sub(self.ln_pad);
    }

    /// Given a file path, try to find a syntax highlighting configuration that matches the path
    /// extension in one of the config directories (`/etc/kibi/syntax.d`, etc.). If such a
    /// configuration is found, set the `syntax` attribute of the editor.
    fn select_syntax_highlight(&mut self, path: &Path) -> Result<(), Error> {
        let extension = path.extension().and_then(std::ffi::OsStr::to_str);
        if let Some(s) = extension.and_then(|e| SyntaxConf::get(e).transpose()) {
            self.syntax = s?;
        }
        Ok(())
    }

    /// Update a row, given its index. If `ignore_following_rows` is `false` and the highlight state
    /// has changed during the update (for instance, it is now in "multi-line comment" state, keep
    /// updating the next rows
    fn update_row(&mut self, y: usize, ignore_following_rows: bool) {
        let mut hl_state = if y > 0 { self.rows[y - 1].hl_state } else { HlState::Normal };
        for row in self.rows.iter_mut().skip(y) {
            let previous_hl_state = row.hl_state;
            hl_state = row.update(&self.syntax, hl_state, self.config.tab_stop);
            if ignore_following_rows || hl_state == previous_hl_state {
                return;
            }
            // If the state has changed (for instance, a multi-line comment started in this row),
            // continue updating the following rows
        }
    }

    /// Update all the rows.
    fn update_all_rows(&mut self) {
        let mut hl_state = HlState::Normal;
        for row in &mut self.rows {
            hl_state = row.update(&self.syntax, hl_state, self.config.tab_stop);
        }
    }

    /// Insert a byte at the current cursor position. If there is no row at the current cursor
    /// position, add a new row and insert the byte.
    fn insert_byte(&mut self, c: u8) {
        if let Some(row) = self.rows.get_mut(self.cursor.y) {
            row.chars.insert(self.cursor.x, c);
        } else {
            self.rows.push(Row::new(vec![c]));
            // The number of rows has changed. The left padding may need to be updated.
            self.update_screen_cols();
        }
        self.update_row(self.cursor.y, false);
        self.cursor.x += 1;
        self.n_bytes += 1;
        self.dirty = true;
    }
}
