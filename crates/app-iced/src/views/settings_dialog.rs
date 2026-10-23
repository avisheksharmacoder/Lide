//! 2-pane Settings modal dialog: Left pane selects category, right pane shows detail view.

use iced::widget::{button, column, container, mouse_area, row, scrollable, text, text_input, Space};
use iced::{Alignment, Color, Element, Length};

use crate::fonts::{FONT_JETBRAINS_MONO, FONT_JETBRAINS_MONO_BOLD};
use crate::icons::{icon, icon_colored, icon_sized_colored, Icon};
use crate::theme::ThemePalette;
use crate::{AiConnectionStatus, EditorCommand, LideApp, SettingsTab, ThemeMode};

/// Renders the 2-pane Settings modal dialog layered over the editor shell.
pub fn render_settings_dialog(app: &LideApp) -> Element<'_, EditorCommand> {
    let theme = &app.current_theme;

    // 1. Header Bar: Title, Icon, and Close 'X' Button
    let header_title = row![
        icon_sized_colored(Icon::Settings, 16, theme.text_accent),
        text("Settings")
            .size(app.ui_title_size())
            .font(FONT_JETBRAINS_MONO_BOLD)
            .color(theme.text_accent),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let close_btn = button(icon_colored(Icon::X, theme.text_muted))
        .on_press(EditorCommand::CloseSettingsDialog)
        .padding([4, 8])
        .style(close_icon_button_style(theme));

    let header = row![
        header_title,
        Space::with_width(Length::Fill),
        close_btn,
    ]
    .padding([10, 16])
    .align_y(Alignment::Center);

    // 2. Left Pane: Category Item List
    let left_pane = container(
        column![
            category_tab_item(
                "Fonts",
                Icon::Type,
                SettingsTab::Fonts,
                app.active_settings_tab,
                app.ui_font_size(),
                theme,
            ),
            category_tab_item(
                "AI Assistant",
                Icon::Sparkles,
                SettingsTab::Ai,
                app.active_settings_tab,
                app.ui_font_size(),
                theme,
            ),
            category_tab_item(
                "Theme",
                Icon::Palette,
                SettingsTab::Theme,
                app.active_settings_tab,
                app.ui_font_size(),
                theme,
            ),
            category_tab_item(
                "Configs",
                Icon::Sliders,
                SettingsTab::Configs,
                app.active_settings_tab,
                app.ui_font_size(),
                theme,
            ),
            category_tab_item(
                "AI Models",
                Icon::Cpu,
                SettingsTab::AiModels,
                app.active_settings_tab,
                app.ui_font_size(),
                theme,
            ),
        ]
        .spacing(4)
        .padding(8),
    )
    .width(220)
    .height(Length::Fill)
    .style(move |_| container::Style {
        background: Some(theme.bg_toolbar.into()),
        border: iced::border::color(theme.border_subtle).width(1.0),
        ..Default::default()
    });

    // 3. Right Pane: Detail View of Selected Category
    let detail_content: Element<'_, EditorCommand> = match app.active_settings_tab {
        SettingsTab::Fonts => render_fonts_detail(app),
        SettingsTab::Ai => render_ai_detail(app),
        SettingsTab::Theme => render_theme_detail(app),
        SettingsTab::Configs => render_configs_detail(app),
        SettingsTab::AiModels => render_ai_models_detail(app),
    };

    let right_pane = container(scrollable(detail_content).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([14, 20]);

    // 4. Modal Body: Left Category List + Right Detail View
    let body = row![left_pane, right_pane]
        .width(Length::Fill)
        .height(Length::Fill);

    // 5. Footer: Shortcut Hint + Done Button
    let footer = row![
        text("Press Esc to close").size(app.ui_caption_size()).color(theme.text_muted),
        Space::with_width(Length::Fill),
        button(text("Done").size(app.ui_font_size()).color(theme.text_primary))
            .on_press(EditorCommand::CloseSettingsDialog)
            .padding([4, 16])
            .style(action_button_style(theme)),
    ]
    .padding([8, 16])
    .align_y(Alignment::Center);

    let border_subtle = theme.border_subtle;

    // 6. Complete Card
    let card = container(
        column![
            header,
            container(Space::with_height(1))
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(border_subtle.into()),
                    ..Default::default()
                }),
            body,
            container(Space::with_height(1))
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(border_subtle.into()),
                    ..Default::default()
                }),
            footer,
        ]
    )
    .width(860)
    .height(620)
    .style(move |_| container::Style {
        background: Some(theme.bg_card.into()),
        border: iced::border::rounded(8).color(theme.border_focused).width(1.0),
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.55),
            offset: iced::Vector::new(0.0, 8.0),
            blur_radius: 20.0,
        },
        ..Default::default()
    });

    // 7. Full-screen backdrop overlay with outside-click dismiss
    mouse_area(
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(|_| container::Style {
                background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.65).into()),
                ..Default::default()
            }),
    )
    .on_press(EditorCommand::CloseSettingsDialog)
    .into()
}

// ---------------------------------------------------------------------------
// Left Category Item Widget
// ---------------------------------------------------------------------------

fn category_tab_item<'a>(
    label: &'static str,
    ic: Icon,
    tab: SettingsTab,
    current_tab: SettingsTab,
    font_size: f32,
    theme: &ThemePalette,
) -> Element<'a, EditorCommand> {
    let is_active = tab == current_tab;
    let text_color = if is_active {
        theme.text_primary
    } else {
        theme.text_muted
    };

    let content = row![
        icon(ic),
        text(label).size(font_size).color(text_color),
        Space::with_width(Length::Fill),
        if is_active {
            icon(Icon::Check)
        } else {
            row![].into()
        },
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let bg_card = theme.bg_card;
    let bg_hover = theme.bg_hover;
    let border_focused = theme.border_focused;
    let text_primary = theme.text_primary;
    let text_muted = theme.text_muted;

    button(content)
        .on_press(EditorCommand::SelectSettingsTab(tab))
        .padding([8, 12])
        .width(Length::Fill)
        .style(move |_, status| {
            if is_active {
                button::Style {
                    background: Some(bg_card.into()),
                    text_color: text_primary,
                    border: iced::border::rounded(5).color(border_focused).width(1.0),
                    ..Default::default()
                }
            } else {
                match status {
                    button::Status::Hovered => button::Style {
                        background: Some(bg_hover.into()),
                        text_color: text_primary,
                        border: iced::border::rounded(5),
                        ..Default::default()
                    },
                    _ => button::Style {
                        background: None,
                        text_color: text_muted,
                        ..Default::default()
                    },
                }
            }
        })
        .into()
}

// ---------------------------------------------------------------------------
// Right Detail Views
// ---------------------------------------------------------------------------

fn render_fonts_detail(app: &LideApp) -> Element<'_, EditorCommand> {
    let theme = &app.current_theme;
    let title = text("Fonts & Typography").size(app.ui_title_size()).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_primary);
    let desc = text("Pure-Rust in-memory font embedding. JetBrains Mono is bundled directly in the binary with zero OS dependencies.")
        .size(app.ui_caption_size())
        .color(theme.text_muted);

    // 1. Entire IDE Label Font Size Controls (Default: 20 px)
    let current_ui_sz = app.ide_font_size;
    let ui_size_btns = row![
        text("Size:").size(app.ui_caption_size()).color(theme.text_muted),
        button(text("-").size(app.ui_caption_size()))
            .on_press(EditorCommand::SetIdeFontSize(current_ui_sz - 1.0))
            .padding([3, 8])
            .style(action_button_style(theme)),
        container(text(format!("{:.0} px", current_ui_sz)).size(app.ui_caption_size()).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_primary))
            .padding([3, 10])
            .style(move |_| container::Style {
                background: Some(theme.bg_toolbar.into()),
                border: iced::border::rounded(4).color(theme.border_subtle).width(1.0),
                ..Default::default()
            }),
        button(text("+").size(app.ui_caption_size()))
            .on_press(EditorCommand::SetIdeFontSize(current_ui_sz + 1.0))
            .padding([3, 8])
            .style(action_button_style(theme)),
        Space::with_width(12),
        // Preset buttons for UI Font Size
        preset_ui_size_btn(14.0, current_ui_sz, theme),
        preset_ui_size_btn(16.0, current_ui_sz, theme),
        preset_ui_size_btn(18.0, current_ui_sz, theme),
        preset_ui_size_btn(20.0, current_ui_sz, theme),
        preset_ui_size_btn(22.0, current_ui_sz, theme),
        preset_ui_size_btn(24.0, current_ui_sz, theme),
        preset_ui_size_btn(28.0, current_ui_sz, theme),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    let ui_labels_section = column![
        text("IDE UI Labels Font Size").size(app.ui_caption_size()).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_primary),
        ui_size_btns,
    ]
    .spacing(6);

    // 2. Font Family Selection Chips
    let font_options = [
        ("JetBrains Mono", "In-Memory TrueType Embedded"),
        ("Cascadia Code", "OS Monospace Fallback"),
        ("Consolas", "System Monospace Fallback"),
    ];

    let font_chips = row(font_options.iter().map(|(font_name, subtext)| {
        let is_sel = app.editor_font == *font_name;
        let bg_card = theme.bg_card;
        let bg_toolbar = theme.bg_toolbar;
        let bg_hover = theme.bg_hover;
        let border_focused = theme.border_focused;
        let border_subtle = theme.border_subtle;
        let text_primary = theme.text_primary;
        let text_accent = theme.text_accent;
        let text_muted = theme.text_muted;
        let badge_success = theme.badge_success;

        button(
            column![
                row![
                    icon(Icon::Type),
                    text(*font_name).size(12).font(FONT_JETBRAINS_MONO_BOLD).color(if is_sel { text_accent } else { text_primary }),
                    Space::with_width(Length::Fill),
                    if is_sel {
                        container(text("Active").size(10).color(badge_success))
                            .padding([1, 6])
                            .style(move |_| container::Style {
                                background: Some(Color::from_rgba(0.2, 0.6, 0.2, 0.2).into()),
                                border: iced::border::rounded(3).color(badge_success).width(1.0),
                                ..Default::default()
                            })
                    } else {
                        container(row![])
                    },
                ]
                .align_y(Alignment::Center)
                .spacing(6),
                text(*subtext).size(10).color(text_muted),
            ]
            .spacing(2)
        )
        .on_press(EditorCommand::SetEditorFont(font_name.to_string()))
        .padding([6, 10])
        .width(Length::FillPortion(1))
        .style(move |_, status| {
            if is_sel {
                button::Style {
                    background: Some(bg_card.into()),
                    border: iced::border::rounded(6).color(border_focused).width(1.0),
                    text_color: text_primary,
                    ..Default::default()
                }
            } else {
                match status {
                    button::Status::Hovered => button::Style {
                        background: Some(bg_hover.into()),
                        border: iced::border::rounded(6).color(border_subtle).width(1.0),
                        text_color: text_primary,
                        ..Default::default()
                    },
                    _ => button::Style {
                        background: Some(bg_toolbar.into()),
                        border: iced::border::rounded(6).color(border_subtle).width(1.0),
                        text_color: text_muted,
                        ..Default::default()
                    },
                }
            }
        })
        .into()
    }))
    .spacing(8);

    let family_section = column![
        text("Code Editor Font Family").size(12).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_primary),
        font_chips,
    ]
    .spacing(6);

    // 2. Font Size Controls
    let current_sz = app.editor_font_size;
    let size_btns = row![
        text("Size:").size(12).color(theme.text_muted),
        button(text("-").size(12)).on_press(EditorCommand::SetFontSize(current_sz - 1.0)).padding([3, 8]).style(action_button_style(theme)),
        container(text(format!("{:.0} px", current_sz)).size(12).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_primary))
            .padding([3, 10])
            .style(move |_| container::Style {
                background: Some(theme.bg_toolbar.into()),
                border: iced::border::rounded(4).color(theme.border_subtle).width(1.0),
                ..Default::default()
            }),
        button(text("+").size(12)).on_press(EditorCommand::SetFontSize(current_sz + 1.0)).padding([3, 8]).style(action_button_style(theme)),
        Space::with_width(12),
        // Preset buttons
        preset_size_btn(11.0, current_sz, theme),
        preset_size_btn(12.0, current_sz, theme),
        preset_size_btn(13.0, current_sz, theme),
        preset_size_btn(14.0, current_sz, theme),
        preset_size_btn(15.0, current_sz, theme),
        preset_size_btn(16.0, current_sz, theme),
        preset_size_btn(18.0, current_sz, theme),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    // 3. Line Height Controls
    let lh = app.line_height_px;
    let lh_controls = row![
        text("Line Height:").size(12).color(theme.text_muted),
        button(text("-").size(12)).on_press(EditorCommand::SetLineHeight(lh - 2.0)).padding([3, 8]).style(action_button_style(theme)),
        container(text(format!("{:.0} px", lh)).size(12).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_primary))
            .padding([3, 10])
            .style(move |_| container::Style {
                background: Some(theme.bg_toolbar.into()),
                border: iced::border::rounded(4).color(theme.border_subtle).width(1.0),
                ..Default::default()
            }),
        button(text("+").size(12)).on_press(EditorCommand::SetLineHeight(lh + 2.0)).padding([3, 8]).style(action_button_style(theme)),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    // 4. Live Syntax Highlighting Preview Box (Dynamically Scaled)
    let preview_sz = app.editor_font_size.clamp(9.0, 22.0);
    let preview_font = crate::fonts::resolve_font_by_name(&app.editor_font);
    let line_spacing = ((app.line_height_px as f32) - preview_sz).clamp(2.0, 16.0);

    let preview_box = container(
        column![
            text("def quick_sort(arr: list[int]) -> list[int]:").size(preview_sz).font(preview_font).color(theme.syntax_function),
            text("    \"\"\"Sub-millisecond AST benchmark.\"\"\"").size(preview_sz).font(preview_font).color(theme.syntax_comment),
            text("    if len(arr) <= 1:").size(preview_sz).font(preview_font).color(theme.syntax_keyword),
            text("        return arr").size(preview_sz).font(preview_font).color(theme.text_primary),
            text("    pivot = arr[len(arr) // 2]").size(preview_sz).font(preview_font).color(theme.text_primary),
            text("    left = [x for x in arr if x < pivot]").size(preview_sz).font(preview_font).color(theme.syntax_string),
            text("    return quick_sort(left) + [pivot]").size(preview_sz).font(preview_font).color(theme.text_accent),
        ]
        .spacing(line_spacing)
        .padding(14),
    )
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(theme.bg_editor.into()),
        border: iced::border::rounded(6).color(theme.border_focused).width(1.0),
        ..Default::default()
    });

    let preview_header = row![
        text("Live Syntax Preview").size(11.5).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_primary),
        Space::with_width(Length::Fill),
        text(format!("{} | {:.0} px / {:.0} px line height", app.editor_font, app.editor_font_size, app.line_height_px))
            .size(10.5)
            .color(theme.text_accent),
    ]
    .align_y(Alignment::Center);

    column![
        title,
        desc,
        Space::with_height(10),
        ui_labels_section,
        Space::with_height(8),
        family_section,
        Space::with_height(8),
        size_btns,
        Space::with_height(6),
        lh_controls,
        Space::with_height(10),
        preview_header,
        Space::with_height(2),
        preview_box,
    ]
    .spacing(6)
    .into()
}

fn preset_ui_size_btn<'a>(size: f32, current: f32, theme: &ThemePalette) -> Element<'a, EditorCommand> {
    let is_selected = (size - current).abs() < 0.1;
    let bg_card = theme.bg_card;
    let bg_hover = theme.bg_hover;
    let border_focused = theme.border_focused;
    let text_primary = theme.text_primary;
    let text_muted = theme.text_muted;

    button(text(format!("{:.0}", size)).size(11).color(if is_selected { text_primary } else { text_muted }))
        .on_press(EditorCommand::SetIdeFontSize(size))
        .padding([2, 6])
        .style(move |_, status| {
            if is_selected {
                button::Style {
                    background: Some(bg_card.into()),
                    border: iced::border::rounded(3).color(border_focused).width(1.0),
                    text_color: text_primary,
                    ..Default::default()
                }
            } else {
                match status {
                    button::Status::Hovered => button::Style {
                        background: Some(bg_hover.into()),
                        border: iced::border::rounded(3),
                        text_color: text_primary,
                        ..Default::default()
                    },
                    _ => button::Style {
                        background: None,
                        text_color: text_muted,
                        ..Default::default()
                    },
                }
            }
        })
        .into()
}

fn preset_size_btn<'a>(size: f32, current: f32, theme: &ThemePalette) -> Element<'a, EditorCommand> {
    let is_selected = (size - current).abs() < 0.1;
    let bg_card = theme.bg_card;
    let bg_hover = theme.bg_hover;
    let border_focused = theme.border_focused;
    let text_primary = theme.text_primary;
    let text_muted = theme.text_muted;

    button(text(format!("{:.0}", size)).size(11).color(if is_selected { text_primary } else { text_muted }))
        .on_press(EditorCommand::SetFontSize(size))
        .padding([2, 6])
        .style(move |_, status| {
            if is_selected {
                button::Style {
                    background: Some(bg_card.into()),
                    border: iced::border::rounded(3).color(border_focused).width(1.0),
                    text_color: text_primary,
                    ..Default::default()
                }
            } else {
                match status {
                    button::Status::Hovered => button::Style {
                        background: Some(bg_hover.into()),
                        border: iced::border::rounded(3),
                        text_color: text_primary,
                        ..Default::default()
                    },
                    _ => button::Style {
                        background: None,
                        text_color: text_muted,
                        ..Default::default()
                    },
                }
            }
        })
        .into()
}

fn render_ai_detail(app: &LideApp) -> Element<'_, EditorCommand> {
    let theme = &app.current_theme;
    let title = text("AI Assistant & Privacy").size(15).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_primary);
    let desc = text("Deterministic local Ollama LLM execution with strict zero-leakage AST scope pruning.")
        .size(11.5)
        .color(theme.text_muted);

    let bg_card = theme.bg_card;
    let bg_toolbar = theme.bg_toolbar;
    let bg_hover = theme.bg_hover;
    let border_subtle = theme.border_subtle;
    let border_focused = theme.border_focused;
    let text_primary = theme.text_primary;
    let _text_muted = theme.text_muted;

    // 1. Connection status badge styling based on app.ai_status
    let (badge_bg, badge_border, badge_text_color, badge_label, status_icon) = match app.ai_status {
        AiConnectionStatus::Connected => (
            Color::from_rgba(0.2, 0.6, 0.2, 0.2),
            theme.badge_success,
            theme.badge_success,
            "Connected",
            Icon::Check,
        ),
        AiConnectionStatus::Checking => (
            Color::from_rgba(0.8, 0.6, 0.1, 0.2),
            theme.badge_warning,
            theme.badge_warning,
            "Checking...",
            Icon::Activity,
        ),
        AiConnectionStatus::Error => (
            Color::from_rgba(0.8, 0.2, 0.2, 0.2),
            theme.badge_error,
            theme.badge_error,
            "Error",
            Icon::Zap,
        ),
        AiConnectionStatus::Disconnected => (
            Color::from_rgba(0.5, 0.5, 0.5, 0.15),
            theme.border_subtle,
            theme.text_muted,
            "Disconnected",
            Icon::Sparkles,
        ),
    };

    let status_badge = container(
        row![
            icon(status_icon),
            text(badge_label)
                .size(11)
                .font(FONT_JETBRAINS_MONO_BOLD)
                .color(badge_text_color),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([3, 10])
    .style(move |_| container::Style {
        background: Some(badge_bg.into()),
        border: iced::border::rounded(10).color(badge_border).width(1.0),
        ..Default::default()
    });

    let is_checking = app.ai_status == AiConnectionStatus::Checking;
    let test_button = button(
        row![
            icon(if is_checking { Icon::Activity } else { Icon::Zap }),
            text(if is_checking { "Checking..." } else { "Test Connection" })
                .size(11.5)
                .font(FONT_JETBRAINS_MONO_BOLD)
                .color(if is_checking { theme.badge_warning } else { text_primary }),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .on_press(EditorCommand::ProbeAiConnection)
    .padding([6, 14])
    .style(move |_, status| {
        let bg = match status {
            button::Status::Hovered => bg_hover,
            _ => bg_card,
        };
        button::Style {
            background: Some(bg.into()),
            border: iced::border::rounded(6)
                .color(if is_checking { theme.badge_warning } else { border_focused })
                .width(1.0),
            text_color: text_primary,
            ..Default::default()
        }
    });

    let endpoint_input = text_input("http://localhost:11434", &app.ai_endpoint)
        .on_input(EditorCommand::SetAiEndpoint)
        .on_submit(EditorCommand::ProbeAiConnection)
        .padding([6, 10])
        .size(12)
        .font(FONT_JETBRAINS_MONO)
        .width(Length::Fill);

    let status_message_row = if let Some(msg) = &app.ai_status_message {
        let msg_color = match app.ai_status {
            AiConnectionStatus::Connected => theme.badge_success,
            AiConnectionStatus::Error => theme.badge_error,
            AiConnectionStatus::Checking => theme.badge_warning,
            AiConnectionStatus::Disconnected => theme.text_muted,
        };
        row![
            text(msg).size(11).color(msg_color),
        ]
    } else {
        row![
            text("Enter the host & port for Ollama, then test connection to probe local models.")
                .size(11)
                .color(theme.text_muted),
        ]
    };

    let endpoint_card = container(
        column![
            row![
                icon(Icon::Sparkles),
                text("Ollama Server Endpoint")
                    .size(12)
                    .font(FONT_JETBRAINS_MONO_BOLD)
                    .color(text_primary),
                Space::with_width(Length::Fill),
                status_badge,
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            Space::with_height(6),
            row![
                endpoint_input,
                test_button,
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            Space::with_height(4),
            status_message_row,
        ]
        .spacing(2)
        .padding(14)
    )
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(bg_toolbar.into()),
        border: iced::border::rounded(8).color(border_subtle).width(1.0),
        ..Default::default()
    });

    let privacy_card = container(
        column![
            row![
                icon(Icon::Zap),
                text("Local AST Privacy Guarantee").size(12).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_primary),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            text("When refactoring or generating code, Lide slices strictly the innermost enclosing AST function or class (EnclosingContext). Entire repository files and environmental variables are never transmitted.")
                .size(11.5)
                .color(theme.text_muted),
        ]
        .spacing(6)
        .padding(12)
    )
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(bg_toolbar.into()),
        border: iced::border::rounded(6).color(border_subtle).width(1.0),
        ..Default::default()
    });

    let streaming_card = container(
        row![
            icon(Icon::FlaskConical),
            column![
                text("Token Streaming Engine").size(12).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_primary),
                text("Native Tokio async tasks stream generated code tokens without blocking the 60 FPS UI render loop.")
                    .size(11)
                    .color(theme.text_muted),
            ]
            .spacing(2),
            Space::with_width(Length::Fill),
            text("Active").size(11.5).color(theme.text_accent),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .padding(12)
    )
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(bg_toolbar.into()),
        border: iced::border::rounded(6).color(border_subtle).width(1.0),
        ..Default::default()
    });

    column![
        title,
        desc,
        Space::with_height(10),
        endpoint_card,
        Space::with_height(8),
        privacy_card,
        Space::with_height(8),
        streaming_card,
    ]
    .spacing(4)
    .into()
}

fn render_theme_detail(app: &LideApp) -> Element<'_, EditorCommand> {
    let theme = &app.current_theme;
    let title = text("Theme & Color Scheme").size(15).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_primary);
    let desc = text("Configure color mode (Dark / Light) and syntax highlighting palettes.")
        .size(11.5)
        .color(theme.text_muted);

    // 1. Color Scheme Mode: Dark Mode vs Light Mode
    let is_dark = app.theme_mode == ThemeMode::Dark;
    let bg_card = theme.bg_card;
    let bg_toolbar = theme.bg_toolbar;
    let bg_hover = theme.bg_hover;
    let border_focused = theme.border_focused;
    let border_subtle = theme.border_subtle;
    let text_primary = theme.text_primary;
    let text_muted = theme.text_muted;

    let dark_mode_btn = button(
        row![
            icon(Icon::Moon),
            text("Dark Mode").size(12).color(if is_dark { text_primary } else { text_muted }),
            Space::with_width(Length::Fill),
            if is_dark { icon(Icon::Check) } else { row![].into() },
        ]
        .spacing(8)
        .align_y(Alignment::Center)
    )
    .on_press(EditorCommand::SetThemeMode(ThemeMode::Dark))
    .padding([8, 14])
    .width(Length::FillPortion(1))
    .style(move |_, status| {
        if is_dark {
            button::Style {
                background: Some(bg_card.into()),
                border: iced::border::rounded(6).color(border_focused).width(1.0),
                text_color: text_primary,
                ..Default::default()
            }
        } else {
            match status {
                button::Status::Hovered => button::Style {
                    background: Some(bg_hover.into()),
                    border: iced::border::rounded(6),
                    text_color: text_primary,
                    ..Default::default()
                },
                _ => button::Style {
                    background: Some(bg_toolbar.into()),
                    border: iced::border::rounded(6).color(border_subtle).width(1.0),
                    text_color: text_muted,
                    ..Default::default()
                },
            }
        }
    });

    let is_light = app.theme_mode == ThemeMode::Light;

    let light_mode_btn = button(
        row![
            icon(Icon::Sun),
            text("Light Mode").size(12).color(if is_light { text_primary } else { text_muted }),
            Space::with_width(Length::Fill),
            if is_light { icon(Icon::Check) } else { row![].into() },
        ]
        .spacing(8)
        .align_y(Alignment::Center)
    )
    .on_press(EditorCommand::SetThemeMode(ThemeMode::Light))
    .padding([8, 14])
    .width(Length::FillPortion(1))
    .style(move |_, status| {
        if is_light {
            button::Style {
                background: Some(bg_card.into()),
                border: iced::border::rounded(6).color(border_focused).width(1.0),
                text_color: text_primary,
                ..Default::default()
            }
        } else {
            match status {
                button::Status::Hovered => button::Style {
                    background: Some(bg_hover.into()),
                    border: iced::border::rounded(6),
                    text_color: text_primary,
                    ..Default::default()
                },
                _ => button::Style {
                    background: Some(bg_toolbar.into()),
                    border: iced::border::rounded(6).color(border_subtle).width(1.0),
                    text_color: text_muted,
                    ..Default::default()
                },
            }
        }
    });

    let mode_toggle_row = row![dark_mode_btn, light_mode_btn].spacing(10);

    // 2. Select Theme list according to active mode
    let dark_themes = ["Lide Original v1", "One Dark", "Dracula", "Nord", "GitHub Dark"];
    let light_themes = ["Lide Original v1", "One Light", "GitHub Light", "Solarized Light", "Quiet Light"];

    let current_themes = if is_dark { dark_themes.as_slice() } else { light_themes.as_slice() };

    let theme_selector = column(
        current_themes.iter().map(|th| {
            let is_sel = app.active_theme_name == *th;
            button(
                row![
                    icon(if is_dark { Icon::Moon } else { Icon::Sun }),
                    text(*th).size(12).color(if is_sel { text_primary } else { text_muted }),
                    Space::with_width(Length::Fill),
                    if is_sel { icon(Icon::Check) } else { row![].into() },
                ]
                .spacing(8)
                .align_y(Alignment::Center)
            )
            .on_press(EditorCommand::SetTheme(th.to_string()))
            .padding([6, 12])
            .width(Length::Fill)
            .style(move |_, status| {
                if is_sel {
                    button::Style {
                        background: Some(bg_card.into()),
                        border: iced::border::rounded(4).color(border_focused).width(1.0),
                        text_color: text_primary,
                        ..Default::default()
                    }
                } else {
                    match status {
                        button::Status::Hovered => button::Style {
                            background: Some(bg_hover.into()),
                            border: iced::border::rounded(4),
                            text_color: text_primary,
                            ..Default::default()
                        },
                        _ => button::Style {
                            background: None,
                            text_color: text_muted,
                            ..Default::default()
                        },
                    }
                }
            })
            .into()
        })
    )
    .spacing(4);

    // 3. Dynamic Swatches matching current active theme!
    let swatches = row![
        swatch_pill("Background", theme.bg_editor, theme),
        swatch_pill("Toolbar", theme.bg_toolbar, theme),
        swatch_pill("Cards", theme.bg_card, theme),
        swatch_pill("Accent", theme.text_accent, theme),
        swatch_pill("Function", theme.syntax_function, theme),
        swatch_pill("Keyword", theme.syntax_keyword, theme),
        swatch_pill("String", theme.syntax_string, theme),
        swatch_pill("Class", theme.syntax_class, theme),
    ]
    .spacing(6);

    column![
        title,
        desc,
        Space::with_height(10),
        text("Color Scheme Mode").size(12).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_primary),
        mode_toggle_row,
        Space::with_height(10),
        text(if is_dark { "Select Dark Theme" } else { "Select Light Theme" }).size(12).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_primary),
        container(theme_selector).padding(6).style(move |_| container::Style {
            background: Some(bg_toolbar.into()),
            border: iced::border::rounded(6).color(border_subtle).width(1.0),
            ..Default::default()
        }),
        Space::with_height(10),
        text("Color Token Swatches").size(12).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_primary),
        swatches,
    ]
    .spacing(4)
    .into()
}

fn swatch_pill<'a>(name: &'static str, color: Color, theme: &ThemePalette) -> Element<'a, EditorCommand> {
    container(
        column![
            container(Space::with_height(16))
                .width(55)
                .style(move |_| container::Style {
                    background: Some(color.into()),
                    border: iced::border::rounded(3),
                    ..Default::default()
                }),
            text(name).size(9.5).color(theme.text_muted),
        ]
        .spacing(3)
        .align_x(Alignment::Center)
    )
    .into()
}

fn render_configs_detail(app: &LideApp) -> Element<'_, EditorCommand> {
    let theme = &app.current_theme;
    let title = text("Editor Configurations").size(15).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_primary);
    let desc = text("Core editing behaviors, formatting rules, and sub-millisecond SLAs.")
        .size(11.5)
        .color(theme.text_muted);

    let btn_format_save = config_toggle_row(
        "Format on Save (Astral Ruff)",
        "Formats document in memory atomically before writing to disk (< 5 ms).",
        app.format_on_save_enabled,
        EditorCommand::ToggleFormatOnSave,
        theme,
    );

    let btn_line_numbers = config_toggle_row(
        "Show Line Numbers",
        "Displays 1-indexed line numbers in gutter with error/warning diagnostic badges.",
        app.line_numbers_enabled,
        EditorCommand::ToggleLineNumbers,
        theme,
    );

    // Live Configuration Preview Box
    let preview_lines = [
        ("1", "def calculate_statistics(data: list[float]) -> dict:"),
        ("2", "    \"\"\"Atomic Astral Ruff formatting pass.\"\"\""),
        ("3", "    return {\"mean\": sum(data) / len(data)}"),
    ];

    let show_gutter = app.line_numbers_enabled;
    let preview_rows: Vec<Element<'_, EditorCommand>> = preview_lines
        .iter()
        .map(|(num, code)| {
            let gutter: Element<'_, EditorCommand> = if show_gutter {
                row![
                    text(format!("{:>3}", num))
                        .size(11.5)
                        .font(FONT_JETBRAINS_MONO)
                        .color(theme.text_muted),
                    text(" │").size(11.5).color(theme.border_subtle),
                ]
                .into()
            } else {
                Space::with_width(4).into()
            };

            row![
                gutter,
                Space::with_width(8),
                text(*code)
                    .size(11.5)
                    .font(FONT_JETBRAINS_MONO)
                    .color(theme.text_primary),
            ]
            .align_y(Alignment::Center)
            .into()
        })
        .collect();

    let format_status = if app.format_on_save_enabled {
        ("⚡ Format on Save: Active (Astral Ruff < 5ms)", theme.badge_success)
    } else {
        ("○ Format on Save: Disabled (Raw bytes written to disk)", theme.text_muted)
    };

    let gutter_status = if app.line_numbers_enabled {
        ("1-Indexed Line Gutter: Enabled", theme.text_accent)
    } else {
        ("Gutter: Hidden (Clean Canvas)", theme.text_muted)
    };

    let live_preview_box = container(
        column![
            row![
                text("Live Canvas & Save Engine Preview").size(11).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_accent),
                Space::with_width(Length::Fill),
                text(format_status.0).size(10.5).color(format_status.1),
                Space::with_width(12),
                text(gutter_status.0).size(10.5).color(gutter_status.1),
            ]
            .align_y(Alignment::Center),
            Space::with_height(4),
            container(column(preview_rows).spacing(3).padding(8))
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(theme.bg_editor.into()),
                    border: iced::border::rounded(4).color(theme.border_focused).width(1.0),
                    ..Default::default()
                }),
        ]
        .spacing(4)
        .padding(10)
    )
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(theme.bg_toolbar.into()),
        border: iced::border::rounded(6).color(theme.border_subtle).width(1.0),
        ..Default::default()
    });

    let sla_badge = container(
        column![
            text("Performance SLAs & Architectural Invariants").size(12).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_accent),
            text("• Cold Start: < 50 ms\n• 10k-Line File Open: < 5 ms\n• Keystroke Latency: < 16 ms (60–120 FPS)\n• Ephemeral AST Analysis: < 20 ms background worker\n• Idle Memory (RSS): 30 MB – 50 MB")
                .size(11)
                .color(theme.text_primary),
        ]
        .spacing(4)
        .padding(12)
    )
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(theme.bg_toolbar.into()),
        border: iced::border::rounded(6).color(theme.border_subtle).width(1.0),
        ..Default::default()
    });

    column![
        title,
        desc,
        Space::with_height(10),
        btn_format_save,
        Space::with_height(6),
        btn_line_numbers,
        Space::with_height(10),
        live_preview_box,
        Space::with_height(8),
        sla_badge,
    ]
    .spacing(4)
    .into()
}

fn config_toggle_row<'a>(
    title: &'static str,
    desc: &'static str,
    enabled: bool,
    msg: EditorCommand,
    theme: &'a ThemePalette,
) -> Element<'a, EditorCommand> {
    button(
        row![
            column![
                text(title).size(12).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_primary),
                text(desc).size(11).color(theme.text_muted),
            ]
            .spacing(2),
            Space::with_width(Length::Fill),
            container(text(if enabled { "Enabled" } else { "Disabled" }).size(11).color(if enabled { theme.badge_success } else { theme.text_muted }))
                .padding([4, 10])
                .style(move |_| container::Style {
                    background: Some(if enabled { Color::from_rgba(0.2, 0.6, 0.2, 0.2).into() } else { Color::from_rgba(0.3, 0.3, 0.3, 0.2).into() }),
                    border: iced::border::rounded(6).color(if enabled { theme.badge_success } else { theme.border_subtle }).width(1.0),
                    ..Default::default()
                }),
        ]
        .spacing(12)
        .align_y(Alignment::Center)
    )
    .on_press(msg)
    .padding([8, 12])
    .width(Length::Fill)
    .style(container_button_style(theme))
    .into()
}

fn render_ai_models_detail(app: &LideApp) -> Element<'_, EditorCommand> {
    let theme = &app.current_theme;
    let title = text("Local AI Models").size(15).font(FONT_JETBRAINS_MONO_BOLD).color(theme.text_primary);
    let desc = text("Select active local LLM model for code completion and inline refactoring.")
        .size(11.5)
        .color(theme.text_muted);

    let models = [
        ("qwen2.5-coder:1.5b", "Fastest (< 50ms latency, leanest memory)", "1.5B"),
        ("qwen2.5-coder:7b", "Balanced intelligence & reasoning", "7B"),
        ("deepseek-coder:1.3b", "Specialized Python & algorithm synthesis", "1.3B"),
        ("codellama:7b", "High-capacity reasoning engine", "7B"),
        ("llama3.2:3b", "Compact polyglot instruction model", "3B"),
    ];

    let bg_card = theme.bg_card;
    let bg_toolbar = theme.bg_toolbar;
    let bg_hover = theme.bg_hover;
    let border_focused = theme.border_focused;
    let border_subtle = theme.border_subtle;
    let text_accent = theme.text_accent;
    let text_primary = theme.text_primary;
    let text_muted = theme.text_muted;
    let badge_success = theme.badge_success;

    let model_list = column(
        models.iter().map(|(model_id, description, size_tag)| {
            let is_sel = app.ai_model_name == *model_id;
            button(
                row![
                    icon(Icon::Cpu),
                    column![
                        row![
                            text(*model_id).size(12).font(FONT_JETBRAINS_MONO_BOLD).color(if is_sel { text_accent } else { text_primary }),
                            Space::with_width(8),
                            container(text(*size_tag).size(9.5).color(text_muted))
                                .padding([1, 5])
                                .style(move |_| container::Style {
                                    background: Some(bg_toolbar.into()),
                                    border: iced::border::rounded(3),
                                    ..Default::default()
                                }),
                        ]
                        .align_y(Alignment::Center),
                        text(*description).size(11).color(text_muted),
                    ]
                    .spacing(2),
                    Space::with_width(Length::Fill),
                    if is_sel {
                        container(text("Active").size(11).color(badge_success))
                            .padding([2, 8])
                            .style(move |_| container::Style {
                                background: Some(Color::from_rgba(0.2, 0.6, 0.2, 0.2).into()),
                                border: iced::border::rounded(4).color(badge_success).width(1.0),
                                ..Default::default()
                            })
                    } else {
                        container(text("Select").size(11).color(text_muted))
                            .padding([2, 8])
                            .style(move |_| container::Style {
                                border: iced::border::rounded(4).color(border_subtle).width(1.0),
                                ..Default::default()
                            })
                    },
                ]
                .spacing(12)
                .align_y(Alignment::Center)
            )
            .on_press(EditorCommand::SetAiModel(model_id.to_string()))
            .padding([8, 12])
            .width(Length::Fill)
            .style(move |_, status| {
                if is_sel {
                    button::Style {
                        background: Some(bg_card.into()),
                        border: iced::border::rounded(6).color(border_focused).width(1.0),
                        text_color: text_primary,
                        ..Default::default()
                    }
                } else {
                    match status {
                        button::Status::Hovered => button::Style {
                            background: Some(bg_hover.into()),
                            border: iced::border::rounded(6).color(border_subtle).width(1.0),
                            text_color: text_primary,
                            ..Default::default()
                        },
                        _ => button::Style {
                            background: Some(bg_toolbar.into()),
                            border: iced::border::rounded(6).color(border_subtle).width(1.0),
                            text_color: text_primary,
                            ..Default::default()
                        },
                    }
                }
            })
            .into()
        })
    )
    .spacing(6);

    column![
        title,
        desc,
        Space::with_height(10),
        model_list,
    ]
    .spacing(4)
    .into()
}

// ---------------------------------------------------------------------------
// Button Styling Helpers
// ---------------------------------------------------------------------------

fn container_button_style(theme: &ThemePalette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    let bg_toolbar = theme.bg_toolbar;
    let bg_hover = theme.bg_hover;
    let border_subtle = theme.border_subtle;

    move |_, status| match status {
        button::Status::Hovered => button::Style {
            background: Some(bg_hover.into()),
            border: iced::border::rounded(6).color(border_subtle).width(1.0),
            ..Default::default()
        },
        _ => button::Style {
            background: Some(bg_toolbar.into()),
            border: iced::border::rounded(6).color(border_subtle).width(1.0),
            ..Default::default()
        },
    }
}

fn action_button_style(theme: &ThemePalette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    let bg_card = theme.bg_card;
    let bg_hover = theme.bg_hover;
    let border_focused = theme.border_focused;
    let text_primary = theme.text_primary;

    move |_, status| {
        let bg = match status {
            button::Status::Hovered => bg_hover,
            _ => bg_card,
        };

        button::Style {
            background: Some(bg.into()),
            text_color: text_primary,
            border: iced::border::rounded(4).color(border_focused).width(1.0),
            ..Default::default()
        }
    }
}

fn close_icon_button_style(theme: &ThemePalette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    let text_muted = theme.text_muted;
    let badge_error = theme.badge_error;

    move |_, status| match status {
        button::Status::Hovered => button::Style {
            background: Some(Color::from_rgba(0.8, 0.2, 0.2, 0.3).into()),
            text_color: badge_error,
            border: iced::border::rounded(4),
            ..Default::default()
        },
        _ => button::Style {
            background: None,
            text_color: text_muted,
            ..Default::default()
        },
    }
}
