//! TDD test suite for Lide IDE settings persistence and status bar AI model widget.

use tempfile::tempdir;

use app_iced::settings::LideSettings;
use app_iced::views::status_bar::render_status_bar;
use app_iced::{EditorCommand, LideApp, SettingsTab, ThemeMode};

#[test]
fn test_settings_roundtrip_serialization() {
    let settings = LideSettings {
        theme_name: "Dracula Dark".to_string(),
        theme_mode: "Dark".to_string(),
        editor_font_size: 16.0,
        ide_font_size: 20.0,
        line_height_px: 26.0,
        format_on_save_enabled: false,
        line_numbers_enabled: false,
        ai_endpoint: "http://10.0.0.5:11434".to_string(),
        ai_model_name: "deepseek-coder:1.3b".to_string(),
        editor_font: "Cascadia Code".to_string(),
    };

    let serialized = serde_json::to_string_pretty(&settings).expect("Serialization failed");
    let deserialized: LideSettings =
        serde_json::from_str(&serialized).expect("Deserialization failed");

    assert_eq!(settings, deserialized);
}

#[test]
fn test_settings_save_and_load_from_path() {
    let temp = tempdir().expect("Failed to create tempdir");
    let file_path = temp.path().join("nested").join("settings.json");

    let initial = LideSettings {
        theme_name: "Nord Deep".to_string(),
        theme_mode: "Dark".to_string(),
        editor_font_size: 18.0,
        ide_font_size: 22.0,
        line_height_px: 28.0,
        format_on_save_enabled: true,
        line_numbers_enabled: true,
        ai_endpoint: "http://localhost:11434".to_string(),
        ai_model_name: "codellama:7b".to_string(),
        editor_font: "Consolas".to_string(),
    };

    initial
        .save_to_path(&file_path)
        .expect("Failed to save settings to path");

    let loaded = LideSettings::load_from_path(&file_path).expect("Failed to load settings from path");
    assert_eq!(initial, loaded);
}

#[test]
fn test_settings_from_app_and_apply_settings() {
    let mut app = LideApp::new_headless();

    // Mutate various settings in app
    let _ = app.dispatch(EditorCommand::SetTheme("GitHub Dark".to_string()));
    let _ = app.dispatch(EditorCommand::SetFontSize(18.0));
    let _ = app.dispatch(EditorCommand::SetIdeFontSize(22.0));
    let _ = app.dispatch(EditorCommand::ToggleFormatOnSave);
    let _ = app.dispatch(EditorCommand::SetAiModel("deepseek-coder:1.3b".to_string()));
    let _ = app.dispatch(EditorCommand::SetEditorFont("Cascadia Code".to_string()));

    // Extract snapshot
    let snapshot = LideSettings::from_app(&app);
    assert_eq!(snapshot.theme_name, "GitHub Dark");
    assert_eq!(snapshot.editor_font_size, 18.0);
    assert_eq!(snapshot.ide_font_size, 22.0);
    assert_eq!(snapshot.format_on_save_enabled, false);
    assert_eq!(snapshot.ai_model_name, "deepseek-coder:1.3b");
    assert_eq!(snapshot.editor_font, "Cascadia Code");

    // Apply snapshot to a fresh app
    let mut app2 = LideApp::new_headless();
    app2.apply_settings(&snapshot);

    assert_eq!(app2.active_theme_name, "GitHub Dark");
    assert_eq!(app2.editor_font_size, 18.0);
    assert_eq!(app2.ide_font_size, 22.0);
    assert_eq!(app2.format_on_save_enabled, false);
    assert_eq!(app2.ai_model_name, "deepseek-coder:1.3b");
    assert_eq!(app2.editor_font, "Cascadia Code");
    assert_eq!(app2.theme_mode, ThemeMode::Dark);
}

#[test]
fn test_status_bar_ai_model_widget_and_settings_navigation() {
    let mut app = LideApp::new_headless();

    // Default model visible
    assert_eq!(app.ai_model_name, "qwen2.5-coder:7b");

    // Render status bar without errors
    let _element = render_status_bar(&app);

    // Clicking status bar AI model button navigates to AiModels tab
    let _ = app.dispatch(EditorCommand::OpenSettingsDialog(SettingsTab::AiModels));
    assert!(app.settings_dialog_open);
    assert_eq!(app.active_settings_tab, SettingsTab::AiModels);

    // Selecting a new model updates status bar text & app state
    let _ = app.dispatch(EditorCommand::SetAiModel("deepseek-coder:1.3b".to_string()));
    assert_eq!(app.ai_model_name, "deepseek-coder:1.3b");

    // Re-render status bar with new model
    let _element = render_status_bar(&app);
}
