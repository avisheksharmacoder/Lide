//! TDD test suite verifying keyboard shortcut mapping and subscription actions in app-iced.

use app_iced::subscriptions::keyboard::{resolve_key_action, resolve_key_event};
use app_iced::{ActiveMenu, EditorCommand, LideApp, MoveDirection};
use iced::keyboard::{key::Named, Key, Modifiers};

#[test]
fn test_global_accelerator_hotkeys() {
    let ctrl = Modifiers::COMMAND;
    let ctrl_shift = Modifiers::COMMAND | Modifiers::SHIFT;

    // Ctrl+S -> SaveActiveFile
    assert_eq!(
        resolve_key_action(Key::Character("s".into()), ctrl),
        Some(EditorCommand::SaveActiveFile)
    );

    // Ctrl+N -> NewDocument
    assert_eq!(
        resolve_key_action(Key::Character("n".into()), ctrl),
        Some(EditorCommand::NewDocument)
    );

    // Ctrl+Z -> Undo
    assert_eq!(
        resolve_key_action(Key::Character("z".into()), ctrl),
        Some(EditorCommand::Undo)
    );

    // Ctrl+Shift+Z -> Redo
    assert_eq!(
        resolve_key_action(Key::Character("z".into()), ctrl_shift),
        Some(EditorCommand::Redo)
    );

    // Ctrl+Y -> Redo
    assert_eq!(
        resolve_key_action(Key::Character("y".into()), ctrl),
        Some(EditorCommand::Redo)
    );

    // Ctrl+, -> ToggleFontDialog
    assert_eq!(
        resolve_key_action(Key::Character(",".into()), ctrl),
        Some(EditorCommand::ToggleFontDialog)
    );

    // Ctrl+Shift+F -> ToggleSearchPanel
    assert_eq!(
        resolve_key_action(Key::Character("f".into()), ctrl_shift),
        Some(EditorCommand::ToggleSearchPanel)
    );

    // Ctrl+Shift+T -> TogglePytestPanel
    assert_eq!(
        resolve_key_action(Key::Character("t".into()), ctrl_shift),
        Some(EditorCommand::TogglePytestPanel)
    );

    // Ctrl+Shift+A -> ToggleAiPane
    assert_eq!(
        resolve_key_action(Key::Character("a".into()), ctrl_shift),
        Some(EditorCommand::ToggleAiPane)
    );

    // Ctrl+I -> ToggleAiPane
    assert_eq!(
        resolve_key_action(Key::Character("i".into()), ctrl),
        Some(EditorCommand::ToggleAiPane)
    );

    // F5 -> TriggerPytestRun
    assert_eq!(
        resolve_key_action(Key::Named(Named::F5), Modifiers::empty()),
        Some(EditorCommand::TriggerPytestRun { target_file: None })
    );

    // Escape -> EditorCommand::Escape
    assert_eq!(
        resolve_key_action(Key::Named(Named::Escape), Modifiers::empty()),
        Some(EditorCommand::Escape)
    );
}

#[test]
fn test_escape_modal_dismissal_precedence() {
    let mut app = LideApp::new_headless();

    // 1. Escape with font dialog open -> closes font dialog
    app.font_dialog_open = true;
    let _ = app.dispatch(EditorCommand::Escape);
    assert!(!app.font_dialog_open);

    // 2. Escape with dropdown menu open -> closes menu
    app.active_menu = ActiveMenu::File;
    let _ = app.dispatch(EditorCommand::Escape);
    assert_eq!(app.active_menu, ActiveMenu::None);

    // 3. Escape with search panel open -> closes search panel
    app.search_panel_open = true;
    let _ = app.dispatch(EditorCommand::Escape);
    assert!(!app.search_panel_open);
}

#[test]
fn test_editor_navigation_and_editing_keys() {
    let empty = Modifiers::empty();

    // Arrows
    assert_eq!(
        resolve_key_action(Key::Named(Named::ArrowUp), empty),
        Some(EditorCommand::MoveCursor(MoveDirection::Up))
    );
    assert_eq!(
        resolve_key_action(Key::Named(Named::ArrowDown), empty),
        Some(EditorCommand::MoveCursor(MoveDirection::Down))
    );
    assert_eq!(
        resolve_key_action(Key::Named(Named::ArrowLeft), empty),
        Some(EditorCommand::MoveCursor(MoveDirection::Left))
    );
    assert_eq!(
        resolve_key_action(Key::Named(Named::ArrowRight), empty),
        Some(EditorCommand::MoveCursor(MoveDirection::Right))
    );
    assert_eq!(
        resolve_key_action(Key::Named(Named::Home), empty),
        Some(EditorCommand::MoveCursor(MoveDirection::LineStart))
    );
    assert_eq!(
        resolve_key_action(Key::Named(Named::End), empty),
        Some(EditorCommand::MoveCursor(MoveDirection::LineEnd))
    );

    // Editing
    assert_eq!(
        resolve_key_action(Key::Named(Named::Enter), empty),
        Some(EditorCommand::InsertNewlineAutoIndent)
    );
    assert_eq!(
        resolve_key_action(Key::Named(Named::Backspace), empty),
        Some(EditorCommand::Backspace)
    );
    assert_eq!(
        resolve_key_action(Key::Named(Named::Tab), empty),
        Some(EditorCommand::IndentLine)
    );
    assert_eq!(
        resolve_key_action(Key::Named(Named::Tab), Modifiers::SHIFT),
        Some(EditorCommand::DedentLine)
    );
}

#[test]
fn test_editor_keys_suppressed_when_search_panel_or_dialog_open() {
    let mut app = LideApp::new_headless();
    let _ = app.dispatch(EditorCommand::LoadSource("hello world".to_string()));

    // When search panel is open, backspace does not mutate editor buffer
    app.search_panel_open = true;
    let _ = app.dispatch(EditorCommand::Backspace);
    assert_eq!(app.buffer.line_text(0), "hello world\n");

    // When font dialog is open, backspace does not mutate editor buffer
    app.search_panel_open = false;
    app.font_dialog_open = true;
    let _ = app.dispatch(EditorCommand::Backspace);
    assert_eq!(app.buffer.line_text(0), "hello world\n");
}

#[test]
fn test_capital_characters_and_shifted_symbols() {
    let empty = Modifiers::empty();
    let shift = Modifiers::SHIFT;

    // 1. Priority A: OS-provided typed text (e.g. from Shift + letter or Caps Lock)
    assert_eq!(
        resolve_key_event(
            Key::Character("a".into()),
            Key::Character("A".into()),
            shift,
            Some("A")
        ),
        Some(EditorCommand::InsertText("A".to_string()))
    );

    assert_eq!(
        resolve_key_event(
            Key::Character("z".into()),
            Key::Character("Z".into()),
            shift,
            Some("Z")
        ),
        Some(EditorCommand::InsertText("Z".to_string()))
    );

    // 2. Priority A: Shifted symbols (e.g. Shift + 1 -> '!', Shift + ; -> ':')
    assert_eq!(
        resolve_key_event(
            Key::Character("1".into()),
            Key::Character("!".into()),
            shift,
            Some("!")
        ),
        Some(EditorCommand::InsertText("!".to_string()))
    );

    assert_eq!(
        resolve_key_event(
            Key::Character(";".into()),
            Key::Character(":".into()),
            shift,
            Some(":")
        ),
        Some(EditorCommand::InsertText(":".to_string()))
    );

    assert_eq!(
        resolve_key_event(
            Key::Character("'".into()),
            Key::Character("\"".into()),
            shift,
            Some("\"")
        ),
        Some(EditorCommand::InsertText("\"".to_string()))
    );

    // 3. Priority B: Fallback to `modified_key` when `text` is None
    assert_eq!(
        resolve_key_event(
            Key::Character("a".into()),
            Key::Character("A".into()),
            shift,
            None
        ),
        Some(EditorCommand::InsertText("A".to_string()))
    );

    // 4. Priority C: Fallback to shift capitalization on base `key`
    assert_eq!(
        resolve_key_action(Key::Character("a".into()), shift),
        Some(EditorCommand::InsertText("A".to_string()))
    );

    assert_eq!(
        resolve_key_action(Key::Character("g".into()), shift),
        Some(EditorCommand::InsertText("G".to_string()))
    );

    // 5. Regular lowercase character insertion remains unaffected
    assert_eq!(
        resolve_key_action(Key::Character("x".into()), empty),
        Some(EditorCommand::InsertText("x".to_string()))
    );

    // 6. Non-printable control characters in `text` are filtered out
    assert_eq!(
        resolve_key_event(
            Key::Named(Named::Escape),
            Key::Named(Named::Escape),
            empty,
            Some("\x1b")
        ),
        Some(EditorCommand::Escape)
    );
}

#[test]
fn test_end_to_end_capital_text_insertion_in_editor_buffer() {
    let mut app = LideApp::new_headless();
    let _ = app.dispatch(EditorCommand::LoadSource("".to_string()));

    // Simulate typing "Hello, World!" with capitals and punctuation
    let text_to_type = ["H", "e", "l", "l", "o", ",", " ", "W", "o", "r", "l", "d", "!"];
    for ch in text_to_type {
        let _ = app.dispatch(EditorCommand::InsertText(ch.to_string()));
    }

    assert_eq!(app.buffer.line_text(0), "Hello, World!\n");
    assert!(app.buffer.is_dirty());
}

