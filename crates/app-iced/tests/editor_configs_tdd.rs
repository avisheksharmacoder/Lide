//! TDD test suite for Lide IDE editor configurations: Format on Save and Line Numbers.

use std::fs;
use tempfile::tempdir;

use app_iced::views::settings_dialog::render_settings_dialog;
use app_iced::{EditorCommand, LideApp, SettingsTab};

#[test]
fn test_format_on_save_toggle_state() {
    let mut app = LideApp::new_headless();

    // Default: Enabled
    assert!(app.format_on_save_enabled);

    // Toggle off
    let _ = app.dispatch(EditorCommand::ToggleFormatOnSave);
    assert!(!app.format_on_save_enabled);

    // Toggle back on
    let _ = app.dispatch(EditorCommand::ToggleFormatOnSave);
    assert!(app.format_on_save_enabled);
}

#[test]
fn test_line_numbers_toggle_state() {
    let mut app = LideApp::new_headless();

    // Default: Enabled
    assert!(app.line_numbers_enabled);

    // Toggle off
    let _ = app.dispatch(EditorCommand::ToggleLineNumbers);
    assert!(!app.line_numbers_enabled);

    // Toggle back on
    let _ = app.dispatch(EditorCommand::ToggleLineNumbers);
    assert!(app.line_numbers_enabled);
}

#[test]
fn test_save_active_file_respects_format_on_save_disabled() {
    let dir = tempdir().expect("Failed to create tempdir");
    let file_path = dir.path().join("unformatted_test.py");

    let raw_code = "def   messy_func(   a ,   b ):\n  return   a+b\n";
    fs::write(&file_path, raw_code).expect("Failed to write initial file");

    let mut app = LideApp::new_headless();
    app.active_file_path = Some(file_path.clone());
    app.load_buffer_text(raw_code.to_string());
    app.is_dirty = true;

    // Disable format on save
    app.format_on_save_enabled = false;

    // Trigger Save
    let _ = app.dispatch(EditorCommand::SaveActiveFile);

    // Verify buffer was NOT reformatted
    let saved_content = fs::read_to_string(&file_path).expect("Failed to read saved file");
    assert_eq!(
        saved_content, raw_code,
        "File should NOT be reformatted when format_on_save_enabled is false"
    );
    assert_eq!(app.source_cache, raw_code);
    assert!(!app.is_dirty);
}

#[test]
fn test_save_active_file_formats_when_enabled() {
    let dir = tempdir().expect("Failed to create tempdir");
    let file_path = dir.path().join("formatted_test.py");

    let raw_code = "def   messy_func(   a ,   b ):\n  return   a+b\n";
    fs::write(&file_path, raw_code).expect("Failed to write initial file");

    let mut app = LideApp::new_headless();
    app.active_file_path = Some(file_path.clone());
    app.load_buffer_text(raw_code.to_string());
    app.is_dirty = true;

    // Enable format on save
    app.format_on_save_enabled = true;

    // Trigger Save
    let _ = app.dispatch(EditorCommand::SaveActiveFile);

    // Verify buffer WAS reformatted with Ruff
    let saved_content = fs::read_to_string(&file_path).expect("Failed to read saved file");
    assert_ne!(
        saved_content, raw_code,
        "File should be reformatted when format_on_save_enabled is true"
    );
    assert!(saved_content.contains("def messy_func(a, b):"));
    assert!(saved_content.contains("    return a + b"));
    assert!(!app.is_dirty);
}

#[test]
fn test_render_configs_detail_headless_all_combinations() {
    let mut app = LideApp::new_headless();
    app.settings_dialog_open = true;
    app.active_settings_tab = SettingsTab::Configs;

    for format_enabled in [true, false] {
        for line_numbers in [true, false] {
            app.format_on_save_enabled = format_enabled;
            app.line_numbers_enabled = line_numbers;

            // Render should never panic
            let _element = render_settings_dialog(&app);
        }
    }
}
