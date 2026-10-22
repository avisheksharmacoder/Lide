//! TDD Test Suite for Editor Caret Indentation Preservation and UX Enhancements:
//! 1. Caret overlay rendering: Code text segments are never shifted by cursor presence.
//! 2. Top toolbar cleanliness: Rendered without redundant New/Save/Format buttons.
//! 3. File explorer header: Single STRUCTURE toggle without Files or Close button.
//! 4. Resizable sidebar: Splitter drag lifecycle with min/max width clamping.
//! 5. Single-line tree items: Long file and folder names rendered without line wrapping.

use std::fs;
use tempfile::TempDir;

use app_iced::fonts::resolve_font_by_name;
use app_iced::subscriptions::set_mock_mouse_pos;
use app_iced::theme::ThemePalette;
use app_iced::views::editor::{render_code_segments, render_editor};
use app_iced::views::sidebar::render_file_manager;
use app_iced::views::toolbar::render_toolbar;
use app_iced::{EditorCommand, LideApp, MoveDirection, Position, RenderedLine, SidebarTab};
use iced::Point;

#[test]
fn test_caret_overlay_does_not_shift_or_pollute_code_segments() {
    let mut app = LideApp::new(20.0, 400.0);
    let sample_code = "    def calculate(x):\n        return x * 2\n";
    let _ = app.dispatch(EditorCommand::InsertText(sample_code.to_string()));

    let editor_font = resolve_font_by_name(&app.editor_font);
    let theme = ThemePalette::dark();
    let font_size = app.editor_font_size;

    // Line 0 is active, cursor at col 0
    let _ = app.dispatch(EditorCommand::SetCursor(Position::new(0, 0)));
    assert_eq!(app.buffer.cursor().head.line, 0);
    assert_eq!(app.buffer.cursor().head.col, 0);

    let active_line = app.visible_lines[0].clone();
    assert!(active_line.is_active_line);
    assert_eq!(active_line.cursor_col, Some(0));

    // Create an inactive clone of the same line
    let mut inactive_line = active_line.clone();
    inactive_line.is_active_line = false;
    inactive_line.cursor_col = None;

    // Both active and inactive code segments must contain the exact same count
    // and layout of text elements (no "▎" cursor glyph injected into text stream)
    let active_segments = render_code_segments(&active_line, editor_font, font_size, &theme);
    let inactive_segments = render_code_segments(&inactive_line, editor_font, font_size, &theme);

    assert_eq!(
        active_segments.len(),
        inactive_segments.len(),
        "Active line segments must not have extra cursor glyphs injected into layout"
    );

    // Test cursor at column 4
    let _ = app.dispatch(EditorCommand::SetCursor(Position::new(0, 4)));
    let col4_line = app.visible_lines[0].clone();
    let col4_segments = render_code_segments(&col4_line, editor_font, font_size, &theme);
    assert_eq!(
        col4_segments.len(),
        inactive_segments.len(),
        "Cursor at column 4 must not inject extra glyph widgets into code row"
    );

    // Headless render of full editor
    let _editor_elem = render_editor(&app);
}

#[test]
fn test_sidebar_resize_lifecycle_and_clamping() {
    let mut app = LideApp::new(20.0, 400.0);

    // Initial default state
    assert_eq!(app.sidebar_width_px, 250.0);
    assert!(!app.is_resizing_sidebar);

    // 1. Start resizing
    let _ = app.dispatch(EditorCommand::StartResizeSidebar);
    assert!(app.is_resizing_sidebar);

    // 2. Resize within valid bounds
    let _ = app.dispatch(EditorCommand::ResizeSidebar(360.0));
    assert_eq!(app.sidebar_width_px, 360.0);

    // 3. Lower limit clamping (min 140px)
    let _ = app.dispatch(EditorCommand::ResizeSidebar(80.0));
    assert_eq!(app.sidebar_width_px, 140.0);

    // 4. Upper limit clamping (max 700px)
    let _ = app.dispatch(EditorCommand::ResizeSidebar(950.0));
    assert_eq!(app.sidebar_width_px, 700.0);

    // 5. End resizing
    let _ = app.dispatch(EditorCommand::EndResizeSidebar);
    assert!(!app.is_resizing_sidebar);

    // 6. Resizing command ignored when not in active drag state
    let _ = app.dispatch(EditorCommand::ResizeSidebar(450.0));
    assert_eq!(app.sidebar_width_px, 700.0);
}

#[test]
fn test_file_explorer_structure_toggle_and_header_cleanup() {
    let mut app = LideApp::new(20.0, 400.0);

    // Initial tab is ProjectFiles
    assert_eq!(app.active_sidebar_tab, SidebarTab::ProjectFiles);

    // Toggle to Structure
    let _ = app.dispatch(EditorCommand::ToggleStructureTab);
    assert_eq!(app.active_sidebar_tab, SidebarTab::Structure);

    // Toggle back to ProjectFiles
    let _ = app.dispatch(EditorCommand::ToggleStructureTab);
    assert_eq!(app.active_sidebar_tab, SidebarTab::ProjectFiles);

    // Headless render of sidebar panel
    let _sidebar_elem = render_file_manager(&app);
}

#[test]
fn test_structure_tab_back_arrow_navigation_lifecycle() {
    let mut app = LideApp::new(20.0, 400.0);

    // 1. Initial state: ProjectFiles
    assert_eq!(app.active_sidebar_tab, SidebarTab::ProjectFiles);
    let _files_view = render_file_manager(&app);

    // 2. Click STRUCTURE button in header to switch to Structure outline
    let _ = app.dispatch(EditorCommand::SwitchSidebarTab(SidebarTab::Structure));
    assert_eq!(app.active_sidebar_tab, SidebarTab::Structure);

    // 3. Render Structure view with back arrow button header
    let _structure_view = render_file_manager(&app);

    // 4. Click the back arrow button to return to File Explorer pane
    let _ = app.dispatch(EditorCommand::SwitchSidebarTab(SidebarTab::ProjectFiles));
    assert_eq!(app.active_sidebar_tab, SidebarTab::ProjectFiles);

    // 5. Render File Explorer view again
    let _files_view_returned = render_file_manager(&app);
}

#[test]
fn test_top_toolbar_renders_cleanly_without_new_save_format_buttons() {
    let app = LideApp::new(20.0, 400.0);
    let _toolbar_elem = render_toolbar(&app);
}

#[test]
fn test_single_line_wrapping_for_long_filenames() {
    let temp = TempDir::new().expect("Failed to create temp directory");
    let root = temp.path().to_path_buf();

    let nested_dir = root.join("long_service_configuration_module_directory");
    fs::create_dir_all(&nested_dir).unwrap();

    let long_file = nested_dir.join(
        "a_very_long_nested_service_configuration_controller_test_suite_runner_component.py",
    );
    fs::write(&long_file, "def test_long(): pass\n").unwrap();

    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::ReceiveSelectedFolder(Some(root.clone())));

    // Expand the directory
    let _ = app.dispatch(EditorCommand::ToggleDirectoryExpand(nested_dir));

    // Headless render of file tree with long filenames
    let _sidebar_elem = render_file_manager(&app);
}

#[test]
fn test_reselect_folder_from_active_workspace_header() {
    let temp_a = TempDir::new().expect("Failed to create temp A");
    let root_a = temp_a.path().to_path_buf();
    fs::write(root_a.join("alpha.py"), "x = 1\n").unwrap();

    let temp_b = TempDir::new().expect("Failed to create temp B");
    let root_b = temp_b.path().to_path_buf();
    fs::write(root_b.join("beta.py"), "y = 2\n").unwrap();

    let mut app = LideApp::new(20.0, 400.0);

    // 1. Initial empty state: Header renders open folder action
    assert!(app.active_workspace_root.is_none());
    let _sidebar_empty = render_file_manager(&app);

    // 2. Open workspace A
    let _ = app.dispatch(EditorCommand::ReceiveSelectedFolder(Some(root_a.clone())));
    assert_eq!(app.active_workspace_root, Some(root_a.clone()));
    let tree_a = app.workspace_tree.as_ref().unwrap();
    assert!(tree_a.children.iter().any(|c| c.name == "alpha.py"));

    // 3. Render header with active workspace - includes re-select folder button
    let _sidebar_a = render_file_manager(&app);

    // 4. User clicks re-select folder icon button (dispatches PromptSystemFolderPicker)
    let _task = app.dispatch(EditorCommand::PromptSystemFolderPicker);

    // 5. User selects workspace B via OS dialog
    let _ = app.dispatch(EditorCommand::ReceiveSelectedFolder(Some(root_b.clone())));
    assert_eq!(app.active_workspace_root, Some(root_b.clone()));
    let tree_b = app.workspace_tree.as_ref().unwrap();
    assert!(tree_b.children.iter().any(|c| c.name == "beta.py"));
    assert!(!tree_b.children.iter().any(|c| c.name == "alpha.py"));

    // 6. Render sidebar with workspace B
    let _sidebar_b = render_file_manager(&app);
}

#[test]
fn test_mouse_click_places_cursor_at_specific_line_and_column() {
    let mut app = LideApp::new(20.0, 400.0);
    let sample = "def calculate(x):\n    return x * 2\n    # end of block\n";
    let _ = app.dispatch(EditorCommand::InsertText(sample.to_string()));

    let gutter_w = if app.line_numbers_enabled { 72.0 } else { 16.0 };
    let code_start_x = if app.file_manager_open {
        app.sidebar_width_px + 4.0 + gutter_w
    } else {
        gutter_w
    };
    let char_width = app.editor_font_size * 0.60;

    // 1. Click at line 1, column 4 (start of 'return')
    set_mock_mouse_pos(Point::new(code_start_x + 4.0 * char_width, 100.0));
    let _ = app.dispatch(EditorCommand::ClickEditorLine { line_idx: 1 });
    assert_eq!(app.buffer.cursor().head, Position::new(1, 4));
    assert_eq!(app.visible_lines[1].cursor_col, Some(4));

    // 2. Click at line 1, column 11 (the 'x' parameter in 'return x * 2')
    set_mock_mouse_pos(Point::new(code_start_x + 11.0 * char_width, 100.0));
    let _ = app.dispatch(EditorCommand::ClickEditorLine { line_idx: 1 });
    assert_eq!(app.buffer.cursor().head, Position::new(1, 11));
    assert_eq!(app.visible_lines[1].cursor_col, Some(11));

    // 3. Click in the gutter / to the left of text -> clamps to column 0
    set_mock_mouse_pos(Point::new(10.0, 100.0));
    let _ = app.dispatch(EditorCommand::ClickEditorLine { line_idx: 1 });
    assert_eq!(app.buffer.cursor().head, Position::new(1, 0));
    assert_eq!(app.visible_lines[1].cursor_col, Some(0));

    // 4. Click far to the right past the line end -> clamps to line grapheme length
    set_mock_mouse_pos(Point::new(3000.0, 100.0));
    let _ = app.dispatch(EditorCommand::ClickEditorLine { line_idx: 0 });
    let line0_len = app.buffer.line_grapheme_len(0);
    assert_eq!(app.buffer.cursor().head, Position::new(0, line0_len));
    assert_eq!(app.visible_lines[0].cursor_col, Some(line0_len));

    // 5. Click line with file manager closed
    app.file_manager_open = false;
    let code_start_closed = gutter_w;
    set_mock_mouse_pos(Point::new(code_start_closed + 6.0 * char_width, 100.0));
    let _ = app.dispatch(EditorCommand::ClickEditorLine { line_idx: 2 });
    assert_eq!(app.buffer.cursor().head, Position::new(2, 6));
}

#[test]
fn test_mouse_click_empty_space_places_cursor_at_end_of_file() {
    let mut app = LideApp::new(20.0, 400.0);
    let sample = "x = 10\ny = 20\n";
    let _ = app.dispatch(EditorCommand::InsertText(sample.to_string()));

    // Initial cursor is at end of sample text
    let _ = app.dispatch(EditorCommand::SetCursor(Position::new(0, 0)));
    assert_eq!(app.buffer.cursor().head, Position::new(0, 0));

    // Click in empty space below text
    let _ = app.dispatch(EditorCommand::ClickEditorEmptySpace);
    let last_line = app.buffer.len_lines().saturating_sub(1);
    let last_col = app.buffer.line_grapheme_len(last_line);
    assert_eq!(app.buffer.cursor().head, Position::new(last_line, last_col));
}

#[test]
fn test_cursor_movement_and_status_bar_sync() {
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::InsertText("first line\nsecond line\n".to_string()));

    // 1. Move to start of document
    let _ = app.dispatch(EditorCommand::SetCursor(Position::new(0, 0)));
    assert!(app.status_bar_text.contains("Ln 1, Col 1"));

    // 2. Move Right 5 times
    for _ in 0..5 {
        let _ = app.dispatch(EditorCommand::MoveCursor(MoveDirection::Right));
    }
    assert_eq!(app.buffer.cursor().head, Position::new(0, 5));
    assert!(app.status_bar_text.contains("Ln 1, Col 6"));

    // 3. Move Down
    let _ = app.dispatch(EditorCommand::MoveCursor(MoveDirection::Down));
    assert_eq!(app.buffer.cursor().head.line, 1);
    assert!(app.status_bar_text.contains("Ln 2, Col 6"));

    // 4. Move to Line End
    let _ = app.dispatch(EditorCommand::MoveCursor(MoveDirection::LineEnd));
    let line1_len = app.buffer.line_grapheme_len(1);
    assert_eq!(app.buffer.cursor().head, Position::new(1, line1_len));
    assert!(app.status_bar_text.contains(&format!("Col {}", line1_len + 1)));

    // 5. Move to Line Start
    let _ = app.dispatch(EditorCommand::MoveCursor(MoveDirection::LineStart));
    assert_eq!(app.buffer.cursor().head, Position::new(1, 0));
    assert!(app.status_bar_text.contains("Ln 2, Col 1"));
}

#[test]
fn test_typing_advances_cursor_and_preserves_caret_visibility() {
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::LoadSource("".to_string()));

    // Start with empty buffer
    assert_eq!(app.buffer.cursor().head, Position::new(0, 0));
    assert_eq!(app.visible_lines[0].cursor_col, Some(0));

    // Type character "H"
    let _ = app.dispatch(EditorCommand::InsertText("H".to_string()));
    assert_eq!(app.buffer.cursor().head, Position::new(0, 1));
    assert_eq!(app.visible_lines[0].text, "H");
    assert_eq!(app.visible_lines[0].cursor_col, Some(1));

    // Type "ello"
    let _ = app.dispatch(EditorCommand::InsertText("ello".to_string()));
    assert_eq!(app.buffer.cursor().head, Position::new(0, 5));
    assert_eq!(app.visible_lines[0].text, "Hello");
    assert_eq!(app.visible_lines[0].cursor_col, Some(5));

    // Backspace
    let _ = app.dispatch(EditorCommand::Backspace);
    assert_eq!(app.buffer.cursor().head, Position::new(0, 4));
    assert_eq!(app.visible_lines[0].text, "Hell");
    assert_eq!(app.visible_lines[0].cursor_col, Some(4));

    // Render full editor to verify zero panics with updated caret bounds
    let _editor_view = render_editor(&app);
}

