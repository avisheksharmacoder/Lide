//! Lide IDE - Pure-Rust Desktop Python & Polyglot IDE
//!
//! Powered by Iced (Native GUI), Astral Ruff (AST & Formatter),
//! Tree-sitter (Incremental Highlighting), and Ripgrep (Search).

use app_iced::fonts::*;
use app_iced::LideApp;
use iced::Size;

fn main() -> iced::Result {
    iced::application("Lide - Python & Polyglot IDE", LideApp::update, LideApp::view)
        .font(JETBRAINS_MONO_MEDIUM_BYTES)
        .font(JETBRAINS_MONO_BOLD_BYTES)
        .font(JETBRAINS_MONO_ITALIC_BYTES)
        .font(JETBRAINS_MONO_BOLD_ITALIC_BYTES)
        .font(lucide_icons::LUCIDE_FONT_BYTES)
        .default_font(FONT_JETBRAINS_MONO)
        .subscription(LideApp::subscription)
        .window_size(Size::new(1280.0, 800.0))
        .run_with(|| {
            let mut app = LideApp::new(22.0, 792.0);
            let saved = app_iced::LideSettings::load();
            app.apply_settings(&saved);
            (app, iced::Task::none())
        })
}
