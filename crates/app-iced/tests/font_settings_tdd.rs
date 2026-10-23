use app_iced::{ActiveMenu, EditorCommand, LideApp};

#[test]
fn test_default_typography_configuration() {
    let app = LideApp::new_headless();

    // Initial default state
    assert_eq!(app.font_dialog_open, false);
    assert_eq!(
        app.ide_font,
        "Quicksand",
        "Default IDE UI font must be Quicksand, got: {}",
        app.ide_font
    );
    assert_eq!(
        app.ide_font_size,
        20.0,
        "Default IDE UI font size must be 20.0, got: {}",
        app.ide_font_size
    );
    assert_eq!(
        app.editor_font,
        "JetBrains Mono",
        "Default Code Editor font must be JetBrains Mono, got: {}",
        app.editor_font
    );
    assert_eq!(
        app.terminal_font,
        "JetBrains Mono",
        "Default Terminal font must be JetBrains Mono, got: {}",
        app.terminal_font
    );
}

#[test]
fn test_toggle_font_dialog_and_menu_dismissal() {
    let mut app = LideApp::new_headless();

    // Open a menu first
    let _ = app.dispatch(EditorCommand::ToggleMenu(ActiveMenu::Settings));
    assert_eq!(app.active_menu, ActiveMenu::Settings);

    // Opening font dialog dismisses active menu
    let _ = app.dispatch(EditorCommand::ToggleFontDialog);
    assert_eq!(app.font_dialog_open, true);
    assert_eq!(app.active_menu, ActiveMenu::None);

    // Toggling again closes the dialog
    let _ = app.dispatch(EditorCommand::ToggleFontDialog);
    assert_eq!(app.font_dialog_open, false);
}

#[test]
fn test_font_selection_reactive_dispatch() {
    let mut app = LideApp::new_headless();

    // Switch IDE font to Segoe UI
    let _ = app.dispatch(EditorCommand::SetIdeFont("Segoe UI".to_string()));
    assert_eq!(app.ide_font, "Segoe UI");

    // Switch Editor font to Cascadia Code
    let _ = app.dispatch(EditorCommand::SetEditorFont("Cascadia Code".to_string()));
    assert_eq!(app.editor_font, "Cascadia Code");

    // Switch Terminal font to Consolas
    let _ = app.dispatch(EditorCommand::SetTerminalFont("Consolas".to_string()));
    assert_eq!(app.terminal_font, "Consolas");

    // Reset back to defaults
    let _ = app.dispatch(EditorCommand::SetIdeFont("Quicksand".to_string()));
    let _ = app.dispatch(EditorCommand::SetEditorFont("JetBrains Mono".to_string()));
    let _ = app.dispatch(EditorCommand::SetTerminalFont("JetBrains Mono".to_string()));

    assert_eq!(app.ide_font, "Quicksand");
    assert_eq!(app.editor_font, "JetBrains Mono");
    assert_eq!(app.terminal_font, "JetBrains Mono");
}

#[test]
fn test_embedded_fonts_in_memory_and_valid() {
    use app_iced::fonts::EMBEDDED_FONTS;

    assert_eq!(EMBEDDED_FONTS.len(), 4);

    for font_bytes in EMBEDDED_FONTS {
        assert!(
            font_bytes.len() > 200_000,
            "Font byte slice must be a genuine TrueType font (size: {} bytes)",
            font_bytes.len()
        );
        // Verify TTF magic number: 0x00010000 or 'OTTO'
        assert!(
            font_bytes.starts_with(&[0x00, 0x01, 0x00, 0x00]) || font_bytes.starts_with(b"OTTO"),
            "Font must have valid TTF/OTF header"
        );
    }
}

#[test]
fn test_2pane_settings_dialog_toggle_and_tab_transitions() {
    use app_iced::SettingsTab;

    let mut app = LideApp::new_headless();
    assert!(!app.settings_dialog_open);
    assert_eq!(app.active_settings_tab, SettingsTab::Fonts);

    // 1. Toggle settings dialog open
    let _ = app.dispatch(EditorCommand::ToggleSettingsDialog);
    assert!(app.settings_dialog_open);
    assert!(app.font_dialog_open);

    // 2. Switch between all 5 categories
    let _ = app.dispatch(EditorCommand::SelectSettingsTab(SettingsTab::Ai));
    assert_eq!(app.active_settings_tab, SettingsTab::Ai);

    let _ = app.dispatch(EditorCommand::SelectSettingsTab(SettingsTab::Theme));
    assert_eq!(app.active_settings_tab, SettingsTab::Theme);

    let _ = app.dispatch(EditorCommand::SelectSettingsTab(SettingsTab::Configs));
    assert_eq!(app.active_settings_tab, SettingsTab::Configs);

    let _ = app.dispatch(EditorCommand::SelectSettingsTab(SettingsTab::AiModels));
    assert_eq!(app.active_settings_tab, SettingsTab::AiModels);

    let _ = app.dispatch(EditorCommand::SelectSettingsTab(SettingsTab::Fonts));
    assert_eq!(app.active_settings_tab, SettingsTab::Fonts);

    // 3. Update configuration preferences
    let _ = app.dispatch(EditorCommand::SetFontSize(16.0));
    assert_eq!(app.editor_font_size, 16.0);

    let _ = app.dispatch(EditorCommand::SetLineHeight(26.0));
    assert_eq!(app.line_height_px, 26.0);

    let _ = app.dispatch(EditorCommand::SetAiModel("deepseek-coder:1.3b".to_string()));
    assert_eq!(app.ai_model_name, "deepseek-coder:1.3b");

    let _ = app.dispatch(EditorCommand::ToggleFormatOnSave);
    assert!(!app.format_on_save_enabled);
    let _ = app.dispatch(EditorCommand::ToggleFormatOnSave);
    assert!(app.format_on_save_enabled);

    let _ = app.dispatch(EditorCommand::ToggleLineNumbers);
    assert!(app.line_numbers_enabled);

    // Test Theme Mode (Dark / Light)
    use app_iced::ThemeMode;
    assert_eq!(app.theme_mode, ThemeMode::Dark);
    let _ = app.dispatch(EditorCommand::SetThemeMode(ThemeMode::Light));
    assert_eq!(app.theme_mode, ThemeMode::Light);
    assert_eq!(app.active_theme_name, "One Light");

    let _ = app.dispatch(EditorCommand::ToggleThemeMode);
    assert_eq!(app.theme_mode, ThemeMode::Dark);
    assert_eq!(app.active_theme_name, "One Dark");

    // 4. Escape dismisses the modal dialog
    let _ = app.dispatch(EditorCommand::Escape);
    assert!(!app.settings_dialog_open);
    assert!(!app.font_dialog_open);
}

#[test]
fn test_render_settings_dialog_all_tabs_headless() {
    use app_iced::views::settings_dialog::render_settings_dialog;
    use app_iced::SettingsTab;

    let mut app = LideApp::new_headless();
    app.settings_dialog_open = true;

    for tab in [
        SettingsTab::Fonts,
        SettingsTab::Ai,
        SettingsTab::Theme,
        SettingsTab::Configs,
        SettingsTab::AiModels,
    ] {
        app.active_settings_tab = tab;
        let _element = render_settings_dialog(&app);
    }
}

#[test]
fn test_ide_font_size_dispatch_and_scaling() {
    let mut app = LideApp::new_headless();

    // Default: 20.0 px
    assert_eq!(app.ide_font_size, 20.0);
    assert_eq!(app.ui_font_size(), 20.0);
    assert_eq!(app.ui_caption_size(), 16.0);
    assert_eq!(app.ui_title_size(), 24.0);

    // Increase UI font size
    let _ = app.dispatch(EditorCommand::SetIdeFontSize(24.0));
    assert_eq!(app.ide_font_size, 24.0);
    assert_eq!(app.ui_font_size(), 24.0);
    assert_eq!(app.ui_caption_size(), 19.0);
    assert_eq!(app.ui_title_size(), 29.0);

    // Test minimum clamping (10.0 px)
    let _ = app.dispatch(EditorCommand::SetIdeFontSize(5.0));
    assert_eq!(app.ide_font_size, 10.0);

    // Test maximum clamping (36.0 px)
    let _ = app.dispatch(EditorCommand::SetIdeFontSize(50.0));
    assert_eq!(app.ide_font_size, 36.0);
}
