//! TDD test suite for Lide IDE dynamic theme engine and real-time palette transitions.

use app_iced::theme::ThemePalette;
use app_iced::{EditorCommand, LideApp, ThemeMode};
use lang_contract::HighlightToken;

#[test]
fn test_theme_palette_resolution_all_variants() {
    // 1. Dark Palettes
    let one_dark = ThemePalette::resolve("One Dark Pro", ThemeMode::Dark);
    assert_eq!(one_dark.name, "One Dark Pro");
    assert_eq!(one_dark.mode, ThemeMode::Dark);
    assert_eq!(one_dark.bg_editor, iced::Color::from_rgb(0.12, 0.13, 0.17));

    let dracula = ThemePalette::resolve("Dracula Dark", ThemeMode::Dark);
    assert_eq!(dracula.name, "Dracula Dark");
    assert_eq!(dracula.mode, ThemeMode::Dark);
    assert_eq!(dracula.bg_editor, iced::Color::from_rgb(0.16, 0.16, 0.21));

    let nord = ThemePalette::resolve("Nord Deep", ThemeMode::Dark);
    assert_eq!(nord.name, "Nord Deep");
    assert_eq!(nord.mode, ThemeMode::Dark);
    assert_eq!(nord.bg_editor, iced::Color::from_rgb(0.18, 0.20, 0.25));

    let gh_dark = ThemePalette::resolve("GitHub Dark", ThemeMode::Dark);
    assert_eq!(gh_dark.name, "GitHub Dark");
    assert_eq!(gh_dark.mode, ThemeMode::Dark);
    assert_eq!(gh_dark.bg_editor, iced::Color::from_rgb(0.05, 0.07, 0.09));

    // 2. Light Palettes
    let one_light = ThemePalette::resolve("One Light", ThemeMode::Light);
    assert_eq!(one_light.name, "One Light");
    assert_eq!(one_light.mode, ThemeMode::Light);
    assert_eq!(one_light.bg_editor, iced::Color::from_rgb(0.98, 0.98, 0.98));

    let gh_light = ThemePalette::resolve("GitHub Light", ThemeMode::Light);
    assert_eq!(gh_light.name, "GitHub Light");
    assert_eq!(gh_light.mode, ThemeMode::Light);
    assert_eq!(gh_light.bg_editor, iced::Color::from_rgb(1.0, 1.0, 1.0));

    let solarized = ThemePalette::resolve("Solarized Light", ThemeMode::Light);
    assert_eq!(solarized.name, "Solarized Light");
    assert_eq!(solarized.mode, ThemeMode::Light);
    assert_eq!(solarized.bg_editor, iced::Color::from_rgb(0.99, 0.96, 0.89));

    let quiet = ThemePalette::resolve("Quiet Light", ThemeMode::Light);
    assert_eq!(quiet.name, "Quiet Light");
    assert_eq!(quiet.mode, ThemeMode::Light);
    assert_eq!(quiet.bg_editor, iced::Color::from_rgb(0.96, 0.96, 0.96));
}

#[test]
fn test_app_state_theme_transitions() {
    let mut app = LideApp::new(22.0, 792.0);

    // Initial state: One Dark Pro
    assert_eq!(app.theme_mode, ThemeMode::Dark);
    assert_eq!(app.active_theme_name, "One Dark Pro");
    assert_eq!(app.current_theme.name, "One Dark Pro");
    assert_eq!(app.current_theme.mode, ThemeMode::Dark);

    // Switch to Dracula Dark
    let _ = app.dispatch(EditorCommand::SetTheme("Dracula Dark".to_string()));
    assert_eq!(app.active_theme_name, "Dracula Dark");
    assert_eq!(app.current_theme.name, "Dracula Dark");
    assert_eq!(app.current_theme.bg_editor, iced::Color::from_rgb(0.16, 0.16, 0.21));

    // Switch to Light Mode
    let _ = app.dispatch(EditorCommand::SetThemeMode(ThemeMode::Light));
    assert_eq!(app.theme_mode, ThemeMode::Light);
    assert_eq!(app.active_theme_name, "One Light");
    assert_eq!(app.current_theme.name, "One Light");
    assert_eq!(app.current_theme.bg_editor, iced::Color::from_rgb(0.98, 0.98, 0.98));

    // Switch to Solarized Light
    let _ = app.dispatch(EditorCommand::SetTheme("Solarized Light".to_string()));
    assert_eq!(app.active_theme_name, "Solarized Light");
    assert_eq!(app.current_theme.name, "Solarized Light");
    assert_eq!(app.current_theme.bg_editor, iced::Color::from_rgb(0.99, 0.96, 0.89));

    // Toggle back to Dark Mode
    let _ = app.dispatch(EditorCommand::ToggleThemeMode);
    assert_eq!(app.theme_mode, ThemeMode::Dark);
    assert_eq!(app.active_theme_name, "One Dark Pro");
    assert_eq!(app.current_theme.name, "One Dark Pro");
}

#[test]
fn test_theme_syntax_token_colors() {
    let one_dark = ThemePalette::resolve("One Dark Pro", ThemeMode::Dark);
    let dracula = ThemePalette::resolve("Dracula Dark", ThemeMode::Dark);

    let kw_onedark = one_dark.token_color(Some(HighlightToken::Keyword));
    let kw_dracula = dracula.token_color(Some(HighlightToken::Keyword));

    // Keywords must have different colors in One Dark vs Dracula
    assert_ne!(kw_onedark, kw_dracula);
    assert_eq!(kw_onedark, one_dark.syntax_keyword);
    assert_eq!(kw_dracula, dracula.syntax_keyword);
}

#[test]
fn test_headless_render_across_all_themes() {
    let mut app = LideApp::new(22.0, 792.0);
    app.load_buffer_text("def hello_lide():\n    print('Sub-millisecond SLA')\n".to_string());

    let themes = [
        ("One Dark Pro", ThemeMode::Dark),
        ("Dracula Dark", ThemeMode::Dark),
        ("Nord Deep", ThemeMode::Dark),
        ("GitHub Dark", ThemeMode::Dark),
        ("One Light", ThemeMode::Light),
        ("GitHub Light", ThemeMode::Light),
        ("Solarized Light", ThemeMode::Light),
        ("Quiet Light", ThemeMode::Light),
    ];

    for (name, mode) in themes {
        let _ = app.dispatch(EditorCommand::SetThemeMode(mode));
        let _ = app.dispatch(EditorCommand::SetTheme(name.to_string()));
        assert_eq!(app.current_theme.name, name);

        // Verify that rendering the view doesn't panic for any theme
        let _element = app.view();
    }
}
