use editor_core::{Position, TextBuffer};

#[test]
fn test_newline_after_python_colon_adds_4_spaces_and_ignores_trailing_comments() {
    let code = "class NeuralNet:\n    def forward(self, x: float) -> float:  # inline comment";
    let mut buffer = TextBuffer::from_str(code);

    // Place cursor at the end of line 1 (col 63, after `# inline comment`)
    buffer.set_cursor(Position::new(1, 63));
    buffer.insert_newline_auto_indent();

    // Line 1 had 4 leading spaces and ended with `:` before `# inline comment` -> Line 2 must have 8 spaces
    let slice = buffer.viewport_slice(0, 3);
    assert_eq!(slice.lines.len(), 3);
    assert_eq!(slice.lines[2].text, "        ");
    assert_eq!(buffer.cursor().head, Position::new(2, 8));

    // A single Undo must remove both the newline and the 8 auto-indented spaces!
    assert!(buffer.undo());
    assert_eq!(buffer.to_string_lossy(), code);
    assert_eq!(buffer.cursor().head, Position::new(1, 63));
}

#[test]
fn test_newline_on_regular_indented_statement_preserves_current_indent_level() {
    let code = "def compute():\n        scaled = x * 2.0";
    let mut buffer = TextBuffer::from_str(code);

    // Place cursor at end of line 1 (8 leading spaces, no trailing colon)
    buffer.set_cursor(Position::new(1, 24));
    buffer.insert_newline_auto_indent();

    let slice = buffer.viewport_slice(0, 3);
    assert_eq!(slice.lines[2].text, "        ");
    assert_eq!(buffer.cursor().head, Position::new(2, 8));
}

#[test]
fn test_tab_indent_and_shift_tab_dedent_preserve_cursor_offset_and_clamp_at_zero() {
    let code = "def main():\n  return 42\n";
    let mut buffer = TextBuffer::from_str(code);

    // Cursor sits on `'r'` of `  return 42` (line 1, col 2)
    buffer.set_cursor(Position::new(1, 2));

    // 1. Tab (`indent_line`) adds 4 spaces at start of line 1 -> 6 spaces total, cursor shifts to col 6
    buffer.indent_line();
    assert_eq!(buffer.viewport_slice(1, 2).lines[0].text, "      return 42");
    assert_eq!(buffer.cursor().head, Position::new(1, 6));

    // 2. Shift+Tab (`dedent_line`) removes 4 leading spaces -> back to 2 spaces, cursor at col 2
    buffer.dedent_line();
    assert_eq!(buffer.viewport_slice(1, 2).lines[0].text, "  return 42");
    assert_eq!(buffer.cursor().head, Position::new(1, 2));

    // 3. Shift+Tab again when only 2 leading spaces exist removes those 2 spaces without underflowing
    buffer.dedent_line();
    assert_eq!(buffer.viewport_slice(1, 2).lines[0].text, "return 42");
    assert_eq!(buffer.cursor().head, Position::new(1, 0));

    // 4. Shift+Tab at 0 leading spaces is a no-op (does not pollute undo stack)
    buffer.dedent_line();
    assert!(buffer.undo());
    assert_eq!(buffer.viewport_slice(1, 2).lines[0].text, "  return 42");
}
