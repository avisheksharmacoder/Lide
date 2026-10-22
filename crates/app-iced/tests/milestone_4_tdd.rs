use app_iced::{EditorCommand, LideApp};
use editor_core::Position;

#[test]
fn test_format_document_updates_buffer_preserves_cursor_and_supports_undo_redo() {
    let messy_code = concat!(
        "class    MatrixOp:\n",
        "    def  apply( self,x:int )->int:\n",
        "        val=x*2+10\n",
        "        return   val\n"
    );

    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::LoadSource(messy_code.to_string()));

    // Place cursor on line 2, column 14 (near the end of `        val=x*2+10`)
    let _ = app.dispatch(EditorCommand::SetCursor(Position::new(2, 14)));
    assert_eq!(app.buffer.cursor().head, Position::new(2, 14));

    // Trigger Ctrl+S / FormatDocument
    let _ = app.dispatch(EditorCommand::FormatDocument);

    assert_eq!(
        app.last_format_error, None,
        "Formatting valid code must clear last_format_error"
    );

    let formatted_rows = &app.visible_lines;
    assert_eq!(formatted_rows[0].text, "class MatrixOp:");
    assert_eq!(formatted_rows[1].text, "    def apply(self, x: int) -> int:");
    assert_eq!(formatted_rows[2].text, "        val = x * 2 + 10");
    assert_eq!(formatted_rows[3].text, "        return val");

    // Cursor must stay on line 2!
    let cursor_after_fmt = app.buffer.cursor().head;
    assert_eq!(cursor_after_fmt.line, 2);
    assert_eq!(cursor_after_fmt.col, 14);

    // Undo (Ctrl+Z) must restore the exact messy code AND original cursor
    let _ = app.dispatch(EditorCommand::Undo);
    let undone_rows = &app.visible_lines;
    assert_eq!(undone_rows[0].text, "class    MatrixOp:");
    assert_eq!(undone_rows[2].text, "        val=x*2+10");
    assert_eq!(app.buffer.cursor().head, Position::new(2, 14));

    // Redo (Ctrl+Shift+Z) must re-apply the Ruff formatting
    let _ = app.dispatch(EditorCommand::Redo);
    let redone_rows = &app.visible_lines;
    assert_eq!(redone_rows[0].text, "class MatrixOp:");
    assert_eq!(redone_rows[2].text, "        val = x * 2 + 10");
}

#[test]
fn test_format_document_is_noop_when_already_formatted_and_does_not_pollute_undo_stack() {
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::LoadSource("x = 1\n".to_string()));

    // Edit 1: Insert messy line `y=2+3`
    let _ = app.dispatch(EditorCommand::SetCursor(Position::new(1, 0)));
    let _ = app.dispatch(EditorCommand::InsertText("y=2+3\n".to_string()));

    // Edit 2: FormatDocument (normalizes `y=2+3` -> `y = 2 + 3`)
    let _ = app.dispatch(EditorCommand::FormatDocument);
    assert_eq!(app.visible_lines[1].text, "y = 2 + 3");

    // Dispatch FormatDocument 3 more times on the already-formatted buffer
    let _ = app.dispatch(EditorCommand::FormatDocument);
    let _ = app.dispatch(EditorCommand::FormatDocument);
    let _ = app.dispatch(EditorCommand::FormatDocument);

    // A SINGLE Undo must immediately revert back to `y=2+3`
    let _ = app.dispatch(EditorCommand::Undo);
    assert_eq!(
        app.visible_lines[1].text, "y=2+3",
        "Repeated FormatDocument on already-formatted code must not push duplicate undo snapshots"
    );
}

#[test]
fn test_format_document_aborts_gracefully_on_syntax_error_preserving_buffer() {
    let broken_code = "def unfinished(x: int,\n    return x\n";
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::LoadSource(broken_code.to_string()));
    let _ = app.dispatch(EditorCommand::SetCursor(Position::new(0, 15)));

    // Trigger FormatDocument on broken code
    let _ = app.dispatch(EditorCommand::FormatDocument);

    // 1. Buffer text and cursor must remain 100% untouched
    let rows = &app.visible_lines;
    assert_eq!(rows[0].text, "def unfinished(x: int,");
    assert_eq!(rows[1].text, "    return x");
    assert_eq!(app.buffer.cursor().head, Position::new(0, 15));

    // 2. `last_format_error` must be populated
    assert!(
        app.last_format_error.is_some(),
        "Expected last_format_error to contain syntax error description"
    );
}
