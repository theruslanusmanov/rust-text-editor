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
