//! TDD test suite for Lide IDE dynamic theme engine and real-time palette transitions.

use app_iced::theme::ThemePalette;
use app_iced::{EditorCommand, LideApp, ThemeMode};
use lang_contract::HighlightToken;

#[test]
fn test_theme_palette_resolution_all_variants() {
    // 1. Dark Palettes (Canonical Open-Source Names + Lide Original v1)
    let lide_dark = ThemePalette::resolve("Lide Original v1", ThemeMode::Dark);
    assert_eq!(lide_dark.name, "Lide Original v1");
    assert_eq!(lide_dark.mode, ThemeMode::Dark);
    assert_eq!(lide_dark.bg_editor, iced::Color::from_rgb8(0x16, 0x18, 0x1D));
    assert_eq!(lide_dark.text_primary, iced::Color::from_rgb8(0xD4, 0xD9, 0xE2));
    assert_eq!(lide_dark.syntax_keyword, iced::Color::from_rgb8(0xA3, 0x8C, 0xEB));
    assert_eq!(lide_dark.syntax_function, iced::Color::from_rgb8(0xE9, 0xB1, 0x6E));
    assert_eq!(lide_dark.syntax_decorator, iced::Color::from_rgb8(0xF5, 0xC5, 0x42));

    // Cerebral Night alias
    assert_eq!(ThemePalette::resolve("Cerebral Night", ThemeMode::Dark).name, "Lide Original v1");

    let one_dark = ThemePalette::resolve("One Dark", ThemeMode::Dark);
    assert_eq!(one_dark.name, "One Dark");
    assert_eq!(one_dark.mode, ThemeMode::Dark);
    assert_eq!(one_dark.bg_editor, iced::Color::from_rgb(0.12, 0.13, 0.17));

    let dracula = ThemePalette::resolve("Dracula", ThemeMode::Dark);
    assert_eq!(dracula.name, "Dracula");
    assert_eq!(dracula.mode, ThemeMode::Dark);
    assert_eq!(dracula.bg_editor, iced::Color::from_rgb(0.16, 0.16, 0.21));

    let nord = ThemePalette::resolve("Nord", ThemeMode::Dark);
    assert_eq!(nord.name, "Nord");
    assert_eq!(nord.mode, ThemeMode::Dark);
    assert_eq!(nord.bg_editor, iced::Color::from_rgb(0.18, 0.20, 0.25));

    let gh_dark = ThemePalette::resolve("GitHub Dark", ThemeMode::Dark);
    assert_eq!(gh_dark.name, "GitHub Dark");
    assert_eq!(gh_dark.mode, ThemeMode::Dark);
    assert_eq!(gh_dark.bg_editor, iced::Color::from_rgb(0.05, 0.07, 0.09));

    // Backwards-compatible aliases
    assert_eq!(ThemePalette::resolve("One Dark Pro", ThemeMode::Dark).name, "One Dark");
    assert_eq!(ThemePalette::resolve("Dracula Dark", ThemeMode::Dark).name, "Dracula");
    assert_eq!(ThemePalette::resolve("Nord Deep", ThemeMode::Dark).name, "Nord");

    // 2. Light Palettes (Canonical Open-Source Names + Lide Original v1)
    let lide_light = ThemePalette::resolve("Lide Original v1", ThemeMode::Light);
    assert_eq!(lide_light.name, "Lide Original v1");
    assert_eq!(lide_light.mode, ThemeMode::Light);
    assert_eq!(lide_light.bg_editor, iced::Color::from_rgb8(0xF9, 0xFA, 0xFC));
    assert_eq!(lide_light.text_primary, iced::Color::from_rgb8(0x36, 0x3C, 0x49));
    assert_eq!(lide_light.syntax_keyword, iced::Color::from_rgb8(0x6D, 0x4B, 0xD6));
    assert_eq!(lide_light.syntax_function, iced::Color::from_rgb8(0xC9, 0x74, 0x1F));
    assert_eq!(lide_light.syntax_decorator, iced::Color::from_rgb8(0xC6, 0x85, 0x12));

    // Cerebral Day alias
    assert_eq!(ThemePalette::resolve("Cerebral Day", ThemeMode::Light).name, "Lide Original v1");

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

    // Initial state: One Dark
    assert_eq!(app.theme_mode, ThemeMode::Dark);
    assert_eq!(app.active_theme_name, "One Dark");
    assert_eq!(app.current_theme.name, "One Dark");
    assert_eq!(app.current_theme.mode, ThemeMode::Dark);

    // Switch to Lide Original v1 (Dark / Cerebral Night)
    let _ = app.dispatch(EditorCommand::SetTheme("Lide Original v1".to_string()));
    assert_eq!(app.active_theme_name, "Lide Original v1");
    assert_eq!(app.current_theme.name, "Lide Original v1");
    assert_eq!(app.current_theme.bg_editor, iced::Color::from_rgb8(0x16, 0x18, 0x1D));

    // Switch to Light Mode - Lide Original v1 remains active with Cerebral Day colors!
    let _ = app.dispatch(EditorCommand::SetThemeMode(ThemeMode::Light));
    assert_eq!(app.theme_mode, ThemeMode::Light);
    assert_eq!(app.active_theme_name, "Lide Original v1");
    assert_eq!(app.current_theme.name, "Lide Original v1");
    assert_eq!(app.current_theme.bg_editor, iced::Color::from_rgb8(0xF9, 0xFA, 0xFC));

    // Switch to Dracula
    let _ = app.dispatch(EditorCommand::SetThemeMode(ThemeMode::Dark));
    let _ = app.dispatch(EditorCommand::SetTheme("Dracula".to_string()));
    assert_eq!(app.active_theme_name, "Dracula");
    assert_eq!(app.current_theme.name, "Dracula");
    assert_eq!(app.current_theme.bg_editor, iced::Color::from_rgb(0.16, 0.16, 0.21));

    // Switch to Light Mode with Dracula active defaults to One Light
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
    assert_eq!(app.active_theme_name, "One Dark");
    assert_eq!(app.current_theme.name, "One Dark");
}

#[test]
fn test_theme_syntax_token_colors() {
    let one_dark = ThemePalette::resolve("One Dark", ThemeMode::Dark);
    let lide_dark = ThemePalette::resolve("Lide Original v1", ThemeMode::Dark);

    let kw_onedark = one_dark.token_color(Some(HighlightToken::Keyword));
    let kw_lide = lide_dark.token_color(Some(HighlightToken::Keyword));

    assert_ne!(kw_onedark, kw_lide);
    assert_eq!(kw_lide, iced::Color::from_rgb8(0xA3, 0x8C, 0xEB));
}

#[test]
fn test_headless_render_across_all_themes() {
    let mut app = LideApp::new(22.0, 792.0);
    app.load_buffer_text("def hello_lide():\n    print('Sub-millisecond SLA')\n".to_string());

    let themes = [
        ("Lide Original v1", ThemeMode::Dark),
        ("One Dark", ThemeMode::Dark),
        ("Dracula", ThemeMode::Dark),
        ("Nord", ThemeMode::Dark),
        ("GitHub Dark", ThemeMode::Dark),
        ("Lide Original v1", ThemeMode::Light),
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
