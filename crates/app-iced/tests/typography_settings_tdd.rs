//! TDD test suite for typography settings: Font sizing, Line heights, and In-Memory Font resolutions.

use app_iced::fonts::{resolve_font_by_name, FONT_CASCADIA_CODE, FONT_CONSOLAS, FONT_JETBRAINS_MONO};
use app_iced::views::settings_dialog::render_settings_dialog;
use app_iced::{EditorCommand, LideApp, SettingsTab};

#[test]
fn test_set_font_size_bounds_and_line_height_sync() {
    let mut app = LideApp::new_headless();

    // Default font size is 14.0 px
    assert_eq!(app.editor_font_size, 14.0);

    // Set valid font size
    let _ = app.dispatch(EditorCommand::SetFontSize(16.0));
    assert_eq!(app.editor_font_size, 16.0);

    // Test minimum clamping (8.0 px)
    let _ = app.dispatch(EditorCommand::SetFontSize(4.0));
    assert_eq!(app.editor_font_size, 8.0);

    // Test maximum clamping (32.0 px)
    let _ = app.dispatch(EditorCommand::SetFontSize(48.0));
    assert_eq!(app.editor_font_size, 32.0);
    // Line height should automatically scale to fit large 32px font size
    assert!(app.line_height_px >= 32.0);
}

#[test]
fn test_set_line_height_bounds() {
    let mut app = LideApp::new_headless();

    let _ = app.dispatch(EditorCommand::SetFontSize(14.0));
    let _ = app.dispatch(EditorCommand::SetLineHeight(24.0));
    assert_eq!(app.line_height_px, 24.0);

    // Test maximum clamping (48.0 px)
    let _ = app.dispatch(EditorCommand::SetLineHeight(96.0));
    assert_eq!(app.line_height_px, 48.0);

    // Line height cannot be smaller than font size
    let _ = app.dispatch(EditorCommand::SetLineHeight(6.0));
    assert!(app.line_height_px >= app.editor_font_size as f64);
}

#[test]
fn test_set_editor_font_family_resolution() {
    let mut app = LideApp::new_headless();

    // Default: JetBrains Mono
    assert_eq!(app.editor_font, "JetBrains Mono");
    assert_eq!(resolve_font_by_name(&app.editor_font), FONT_JETBRAINS_MONO);

    // Switch to Cascadia Code
    let _ = app.dispatch(EditorCommand::SetEditorFont("Cascadia Code".to_string()));
    assert_eq!(app.editor_font, "Cascadia Code");
    assert_eq!(resolve_font_by_name(&app.editor_font), FONT_CASCADIA_CODE);

    // Switch to Consolas
    let _ = app.dispatch(EditorCommand::SetEditorFont("Consolas".to_string()));
    assert_eq!(app.editor_font, "Consolas");
    assert_eq!(resolve_font_by_name(&app.editor_font), FONT_CONSOLAS);

    // Switch back to JetBrains Mono
    let _ = app.dispatch(EditorCommand::SetEditorFont("JetBrains Mono".to_string()));
    assert_eq!(app.editor_font, "JetBrains Mono");
    assert_eq!(resolve_font_by_name(&app.editor_font), FONT_JETBRAINS_MONO);
}

#[test]
fn test_render_fonts_detail_all_font_sizes_headless() {
    let mut app = LideApp::new_headless();
    app.settings_dialog_open = true;
    app.active_settings_tab = SettingsTab::Fonts;

    let font_sizes = [8.0, 11.0, 12.0, 14.0, 16.0, 18.0, 24.0, 32.0];
    let fonts = ["JetBrains Mono", "Cascadia Code", "Consolas"];

    for font in fonts {
        let _ = app.dispatch(EditorCommand::SetEditorFont(font.to_string()));
        for size in font_sizes {
            let _ = app.dispatch(EditorCommand::SetFontSize(size));
            let _ = app.dispatch(EditorCommand::SetLineHeight((size * 1.5).round()));

            // Headless rendering of the Fonts dialog tab should never panic
            let _element = render_settings_dialog(&app);
        }
    }
}
