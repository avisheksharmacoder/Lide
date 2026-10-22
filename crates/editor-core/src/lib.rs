use std::borrow::Cow;
use std::io::Read;
use std::ops::Range;
use ropey::{Rope, RopeSlice};
use unicode_segmentation::UnicodeSegmentation;

/// Zero-indexed line and grapheme-cluster column coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
    pub line: usize,
    pub col: usize,
}

impl Position {
    pub const fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

/// Direction for single-step cursor movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDirection {
    Up,
    Down,
    Left,
    Right,
    LineStart,
    LineEnd,
}

/// Represents a primary or secondary cursor with "sticky column" memory.
/// `preferred_col` remembers the horizontal target when crossing blank lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cursor {
    pub head: Position,
    pub anchor: Position,
    pub preferred_col: usize,
}

impl Cursor {
    pub fn new(line: usize, col: usize) -> Self {
        let pos = Position::new(line, col);
        Self {
            head: pos,
            anchor: pos,
            preferred_col: col,
        }
    }
}

/// A single materialized line inside the visible GUI viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleLine {
    /// Zero-based line index in the document.
    pub line_idx: usize,
    /// Exact UTF-8 byte range in the underlying Rope (used by Tree-sitter in Milestone 2).
    pub byte_range: Range<usize>,
    /// Line text without trailing CR/LF characters for clean glyph rendering.
    pub text: String,
}

/// The payload sent to Floem's `virtual_stack` on scroll or edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewportSlice {
    pub start_line: usize,
    pub end_line: usize,
    pub total_lines: usize,
    /// Combined UTF-8 byte range of all lines in this viewport slice.
    pub byte_range: Range<usize>,
    pub lines: Vec<VisibleLine>,
}

/// O(1) structural-sharing snapshot of buffer state for Undo/Redo.
#[derive(Clone)]
struct EditorSnapshot {
    rope: Rope,
    cursor: Cursor,
}

/// Core headless buffer backed by a UTF-8 B-tree Rope and structural-sharing Undo/Redo stacks.
pub struct TextBuffer {
    rope: Rope,
    cursor: Cursor,
    undo_stack: Vec<EditorSnapshot>,
    redo_stack: Vec<EditorSnapshot>,
}

impl TextBuffer {
    /// Creates an empty buffer.
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            cursor: Cursor::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Streams a file or reader directly into B-tree Rope chunks without full-file String allocation.
    pub fn from_reader<R: Read>(reader: R) -> std::io::Result<Self> {
        let rope = Rope::from_reader(reader)?;
        Ok(Self {
            rope,
            cursor: Cursor::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    /// Initializes a buffer from an in-memory string slice.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            cursor: Cursor::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Total number of lines in the document (O(1) via Rope root metadata).
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// Total UTF-8 bytes in the document (O(1) via Rope root metadata).
    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    /// Returns the current cursor state.
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Sets the cursor position, clamping to valid line and grapheme boundaries.
    pub fn set_cursor(&mut self, pos: Position) {
        let max_line = self.rope.len_lines().saturating_sub(1);
        let line = pos.line.min(max_line);
        let max_col = self.line_grapheme_len(line);
        let col = pos.col.min(max_col);

        let clamped = Position::new(line, col);
        self.cursor = Cursor {
            head: clamped,
            anchor: clamped,
            preferred_col: col,
        };
    }

    /// Moves the cursor by one logical step while preserving UTF-8 grapheme boundaries
    /// and sticky horizontal columns across short/empty lines.
    pub fn move_cursor(&mut self, dir: MoveDirection) {
        let head = self.cursor.head;
        let max_line = self.rope.len_lines().saturating_sub(1);

        match dir {
            MoveDirection::Up => {
                if head.line > 0 {
                    let target_line = head.line - 1;
                    let max_col = self.line_grapheme_len(target_line);
                    let new_col = self.cursor.preferred_col.min(max_col);
                    self.cursor.head = Position::new(target_line, new_col);
                    self.cursor.anchor = self.cursor.head;
                }
            }
            MoveDirection::Down => {
                if head.line < max_line {
                    let target_line = head.line + 1;
                    let max_col = self.line_grapheme_len(target_line);
                    let new_col = self.cursor.preferred_col.min(max_col);
                    self.cursor.head = Position::new(target_line, new_col);
                    self.cursor.anchor = self.cursor.head;
                }
            }
            MoveDirection::Left => {
                if head.col > 0 {
                    let new_col = head.col - 1;
                    self.cursor.head = Position::new(head.line, new_col);
                } else if head.line > 0 {
                    let prev_line = head.line - 1;
                    let prev_max_col = self.line_grapheme_len(prev_line);
                    self.cursor.head = Position::new(prev_line, prev_max_col);
                }
                self.cursor.anchor = self.cursor.head;
                self.cursor.preferred_col = self.cursor.head.col;
            }
            MoveDirection::Right => {
                let line_len = self.line_grapheme_len(head.line);
                if head.col < line_len {
                    let new_col = head.col + 1;
                    self.cursor.head = Position::new(head.line, new_col);
                } else if head.line < max_line {
                    self.cursor.head = Position::new(head.line + 1, 0);
                }
                self.cursor.anchor = self.cursor.head;
                self.cursor.preferred_col = self.cursor.head.col;
            }
            MoveDirection::LineStart => {
                self.cursor.head = Position::new(head.line, 0);
                self.cursor.anchor = self.cursor.head;
                self.cursor.preferred_col = 0;
            }
            MoveDirection::LineEnd => {
                let max_col = self.line_grapheme_len(head.line);
                self.cursor.head = Position::new(head.line, max_col);
                self.cursor.anchor = self.cursor.head;
                self.cursor.preferred_col = max_col;
            }
        }
    }

    /// Extracts strictly the visible lines `[start_line..end_line)` in O(log N + K) time.
    /// Automatically clamps `end_line` if it exceeds `len_lines()`.
    pub fn viewport_slice(&self, start_line: usize, end_line: usize) -> ViewportSlice {
        let total_lines = self.rope.len_lines();
        let clamped_start = start_line.min(total_lines);
        let clamped_end = end_line.min(total_lines).max(clamped_start);
        let count = clamped_end - clamped_start;

        let slice_start_byte = self.rope.line_to_byte(clamped_start);
        let slice_end_byte = self.rope.line_to_byte(clamped_end);

        let mut lines = Vec::with_capacity(count);
        let mut current_byte = slice_start_byte;

        // `lines_at` performs a single O(log N) tree seek and then streams leaf chunks sequentially.
        for (offset, line_slice) in self.rope.lines_at(clamped_start).take(count).enumerate() {
            let line_idx = clamped_start + offset;
            let next_byte = current_byte + line_slice.len_bytes();

            let cow = slice_to_cow(line_slice);
            let text = strip_line_ending(&cow).to_owned();

            lines.push(VisibleLine {
                line_idx,
                byte_range: current_byte..next_byte,
                text,
            });

            current_byte = next_byte;
        }

        ViewportSlice {
            start_line: clamped_start,
            end_line: clamped_end,
            total_lines,
            byte_range: slice_start_byte..slice_end_byte,
            lines,
        }
    }

    /// Inserts text at the current cursor head, advances the cursor, and records an undo snapshot.
    pub fn insert_at_cursor(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        self.record_undo_snapshot();

        let insert_byte = self.position_to_byte(self.cursor.head);
        let insert_char = self.rope.byte_to_char(insert_byte);

        self.rope.insert(insert_char, text);

        let new_byte = insert_byte + text.len();
        let new_pos = self.byte_to_position(new_byte);
        self.cursor = Cursor {
            head: new_pos,
            anchor: new_pos,
            preferred_col: new_pos.col,
        };
    }

    /// Deletes the character (grapheme cluster) or line break immediately preceding the cursor.
    pub fn delete_backward(&mut self) {
        if self.cursor.head.line == 0 && self.cursor.head.col == 0 {
            return;
        }

        self.record_undo_snapshot();

        let end_byte = self.position_to_byte(self.cursor.head);
        let target_pos = if self.cursor.head.col > 0 {
            Position::new(self.cursor.head.line, self.cursor.head.col - 1)
        } else {
            let prev_line = self.cursor.head.line - 1;
            Position::new(prev_line, self.line_grapheme_len(prev_line))
        };

        let start_byte = self.position_to_byte(target_pos);
        let start_char = self.rope.byte_to_char(start_byte);
        let end_char = self.rope.byte_to_char(end_byte);

        self.rope.remove(start_char..end_char);

        self.cursor = Cursor {
            head: target_pos,
            anchor: target_pos,
            preferred_col: target_pos.col,
        };
    }

    /// Reverts the buffer and cursor to the state prior to the last edit in O(1) time via Rope cloning.
    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(EditorSnapshot {
                rope: self.rope.clone(),
                cursor: self.cursor,
            });
            self.rope = prev.rope;
            self.cursor = prev.cursor;
            true
        } else {
            false
        }
    }

    /// Re-applies the most recently undone edit.
    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(EditorSnapshot {
                rope: self.rope.clone(),
                cursor: self.cursor,
            });
            self.rope = next.rope;
            self.cursor = next.cursor;
            true
        } else {
            false
        }
    }

    /// Returns the full document text (used strictly for test verification).
    pub fn to_string_lossy(&self) -> String {
        self.rope.to_string()
    }

    /// Replaces the entire buffer with `new_text` while preserving/clamping the cursor position.
    /// Returns `true` if the buffer content changed (and an undo snapshot was recorded),
    /// or `false` if `new_text` was identical to the current buffer (no-op).
    pub fn replace_full_preserving_cursor(&mut self, new_text: &str) -> bool {
        if self.rope.len_bytes() == new_text.len() && self.rope == new_text {
            return false;
        }

        self.record_undo_snapshot();

        let prev_head = self.cursor.head;
        let prev_preferred_col = self.cursor.preferred_col;

        self.rope = Rope::from_str(new_text);

        let max_line = self.rope.len_lines().saturating_sub(1);
        let clamped_line = prev_head.line.min(max_line);
        let max_col = self.line_grapheme_len(clamped_line);
        let clamped_col = prev_head.col.min(max_col);

        let new_pos = Position::new(clamped_line, clamped_col);
        self.cursor = Cursor {
            head: new_pos,
            anchor: new_pos,
            preferred_col: prev_preferred_col.min(max_col),
        };

        true
    }

    /// Surgically replaces lines `[start_line..=end_line_inclusive]` with `replacement`,
    /// preserving surrounding lines, clamping the cursor to `(start_line, 0)`, and
    /// recording an O(1) structural-sharing undo snapshot.
    pub fn replace_line_range(
        &mut self,
        start_line: usize,
        end_line_inclusive: usize,
        replacement: &str,
    ) -> bool {
        let total_lines = self.rope.len_lines();
        if total_lines == 0 || start_line >= total_lines {
            return false;
        }

        let clamped_start = start_line.min(total_lines - 1);
        let clamped_end = end_line_inclusive.min(total_lines - 1).max(clamped_start);

        self.record_undo_snapshot();

        let start_byte = self.rope.line_to_byte(clamped_start);
        let end_line_start_byte = self.rope.line_to_byte(clamped_end);
        let end_line_slice = self.rope.line(clamped_end);
        let end_line_cow = slice_to_cow(end_line_slice);
        let end_line_stripped_len = strip_line_ending(&end_line_cow).len();
        let end_byte = end_line_start_byte + end_line_stripped_len;

        let start_char = self.rope.byte_to_char(start_byte);
        let end_char = self.rope.byte_to_char(end_byte);

        let clean_replacement = strip_line_ending(replacement);
        self.rope.remove(start_char..end_char);
        self.rope.insert(start_char, clean_replacement);

        let new_pos = Position::new(clamped_start, 0);
        self.cursor = Cursor {
            head: new_pos,
            anchor: new_pos,
            preferred_col: 0,
        };

        true
    }

    /// Inserts a newline at the cursor and automatically indents the new line according to
    /// Python block rules (adding +4 spaces if the current line prefix ends with `:`).
    pub fn insert_newline_auto_indent(&mut self) {
        let line_idx = self.cursor.head.line;
        let col = self.cursor.head.col;

        let slice = self.rope.line(line_idx);
        let cow = slice_to_cow(slice);
        let stripped = strip_line_ending(&cow);

        // Count leading spaces on the current line
        let leading_spaces = stripped.bytes().take_while(|&b| b == b' ').count();

        // Extract text up to the cursor column and strip any inline `# comment`
        let prefix_byte_end = stripped
            .grapheme_indices(true)
            .nth(col)
            .map(|(idx, _)| idx)
            .unwrap_or(stripped.len());
        let before_cursor = &stripped[..prefix_byte_end];
        let code_without_comment = before_cursor.split('#').next().unwrap_or(before_cursor);

        let extra_indent = if code_without_comment.trim_end().ends_with(':') {
            4
        } else {
            0
        };
        let target_indent = leading_spaces + extra_indent;

        let mut insertion = String::with_capacity(1 + target_indent);
        insertion.push('\n');
        for _ in 0..target_indent {
            insertion.push(' ');
        }

        self.insert_at_cursor(&insertion);
    }

    /// Indents the current line by 4 spaces (PEP 8 Tab) and shifts the cursor right by 4 columns.
    pub fn indent_line(&mut self) {
        self.record_undo_snapshot();

        let line_idx = self.cursor.head.line;
        let line_start_char = self.rope.line_to_char(line_idx);
        self.rope.insert(line_start_char, "    ");

        let new_col = self.cursor.head.col + 4;
        let new_pos = Position::new(line_idx, new_col);
        self.cursor = Cursor {
            head: new_pos,
            anchor: new_pos,
            preferred_col: new_col,
        };
    }

    /// Removes up to 4 leading spaces from the current line (`Shift+Tab`) and shifts the cursor left.
    pub fn dedent_line(&mut self) {
        let line_idx = self.cursor.head.line;
        let slice = self.rope.line(line_idx);
        let cow = slice_to_cow(slice);
        let stripped = strip_line_ending(&cow);

        let leading_spaces = stripped.bytes().take_while(|&b| b == b' ').count();
        let remove_count = leading_spaces.min(4);
        if remove_count == 0 {
            return;
        }

        self.record_undo_snapshot();

        let line_start_char = self.rope.line_to_char(line_idx);
        self.rope
            .remove(line_start_char..(line_start_char + remove_count));

        let new_col = self.cursor.head.col.saturating_sub(remove_count);
        let new_pos = Position::new(line_idx, new_col);
        self.cursor = Cursor {
            head: new_pos,
            anchor: new_pos,
            preferred_col: new_col,
        };
    }

    // ========================================================================
    // INTERNAL O(log N) GRAPHEME & BYTE COORDINATE HELPERS
    // ========================================================================

    fn record_undo_snapshot(&mut self) {
        self.undo_stack.push(EditorSnapshot {
            rope: self.rope.clone(),
            cursor: self.cursor,
        });
        self.redo_stack.clear();
    }

    /// Returns the number of extended Unicode grapheme clusters on `line_idx` (excluding CR/LF).
    pub fn line_grapheme_len(&self, line_idx: usize) -> usize {
        let slice = self.rope.line(line_idx);
        let cow = slice_to_cow(slice);
        strip_line_ending(&cow).graphemes(true).count()
    }

    /// Converts a `(line, grapheme_col)` coordinate to an exact UTF-8 byte offset in the Rope.
    fn position_to_byte(&self, pos: Position) -> usize {
        let max_line = self.rope.len_lines().saturating_sub(1);
        let line = pos.line.min(max_line);
        let line_start_byte = self.rope.line_to_byte(line);

        if pos.col == 0 {
            return line_start_byte;
        }

        let slice = self.rope.line(line);
        let cow = slice_to_cow(slice);
        let stripped = strip_line_ending(&cow);

        let byte_offset_in_line = stripped
            .grapheme_indices(true)
            .nth(pos.col)
            .map(|(byte_idx, _)| byte_idx)
            .unwrap_or(stripped.len());

        line_start_byte + byte_offset_in_line
    }

    /// Converts a UTF-8 byte offset in the Rope back into a `(line, grapheme_col)` coordinate.
    fn byte_to_position(&self, byte_idx: usize) -> Position {
        let clamped_byte = byte_idx.min(self.rope.len_bytes());
        let line = self.rope.byte_to_line(clamped_byte);
        let line_start_byte = self.rope.line_to_byte(line);
        let rel_byte = clamped_byte.saturating_sub(line_start_byte);

        let slice = self.rope.line(line);
        let cow = slice_to_cow(slice);
        let stripped = strip_line_ending(&cow);
        let safe_rel_byte = rel_byte.min(stripped.len());

        let prefix = stripped.get(..safe_rel_byte).unwrap_or(stripped);
        let col = prefix.graphemes(true).count();

        Position::new(line, col)
    }
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Zero-copy conversion when a RopeSlice sits inside a single B-tree chunk (~99% of code lines),
/// falling back to allocation only if a line straddles a chunk boundary.
#[inline]
fn slice_to_cow(slice: RopeSlice<'_>) -> Cow<'_, str> {
    match slice.as_str() {
        Some(s) => Cow::Borrowed(s),
        None => Cow::Owned(slice.to_string()),
    }
}

/// Strips trailing `\r\n`, `\n`, or `\r` without allocating.
#[inline]
fn strip_line_ending(s: &str) -> &str {
    let s = s.strip_suffix('\n').unwrap_or(s);
    s.strip_suffix('\r').unwrap_or(s)
}
