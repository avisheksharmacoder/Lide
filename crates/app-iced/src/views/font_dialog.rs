//! Font Settings modal dialog view for configuring IDE, Editor, and Terminal fonts.

use iced::widget::{button, column, container, row, text, Space};
use iced::{Alignment, Color, Element, Length};

use crate::fonts::resolve_font_by_name;
use crate::icons::{icon_colored, Icon};
use crate::theme::*;
use crate::{EditorCommand, LideApp};

pub fn render_font_dialog(app: &LideApp) -> Element<'_, EditorCommand> {
    let theme = &app.current_theme;
    let title = text("Typography & Font Settings")
        .size(15)
        .color(theme.text_primary);

    let btn_close = button(icon_colored(Icon::X, theme.text_muted))
        .on_press(EditorCommand::ToggleFontDialog)
        .padding([4, 8])
        .style(empty_btn_style);

    let header = row![title, Space::with_width(Length::Fill), btn_close]
        .align_y(Alignment::Center)
        .padding([0, 0]);

    // Section 1: IDE UI Font
    let sec1_title = text("1. IDE UI Font").size(12.5).color(TEXT_ACCENT);
    let sec1_chips = row![
        font_chip("Quicksand", "Quicksand", &app.ide_font, EditorCommand::SetIdeFont("Quicksand".into())),
        font_chip("Segoe UI", "Segoe UI", &app.ide_font, EditorCommand::SetIdeFont("Segoe UI".into())),
        font_chip("JetBrains Mono", "JetBrains Mono", &app.ide_font, EditorCommand::SetIdeFont("JetBrains Mono".into())),
    ]
    .spacing(8);

    // Section 2: Editor Font
    let sec2_title = text("2. Code Editor Font").size(12.5).color(TEXT_ACCENT);
    let sec2_chips = row![
        font_chip("JetBrains Mono", "JetBrains Mono", &app.editor_font, EditorCommand::SetEditorFont("JetBrains Mono".into())),
        font_chip("Cascadia Code", "Cascadia Code", &app.editor_font, EditorCommand::SetEditorFont("Cascadia Code".into())),
        font_chip("Consolas", "Consolas", &app.editor_font, EditorCommand::SetEditorFont("Consolas".into())),
    ]
    .spacing(8);

    // Section 3: Terminal Font
    let sec3_title = text("3. Terminal Font").size(12.5).color(TEXT_ACCENT);
    let sec3_chips = row![
        font_chip("JetBrains Mono", "JetBrains Mono", &app.terminal_font, EditorCommand::SetTerminalFont("JetBrains Mono".into())),
        font_chip("Cascadia Code", "Cascadia Code", &app.terminal_font, EditorCommand::SetTerminalFont("Cascadia Code".into())),
        font_chip("Consolas", "Consolas", &app.terminal_font, EditorCommand::SetTerminalFont("Consolas".into())),
    ]
    .spacing(8);

    // Live Preview Box
    let preview_font = resolve_font_by_name(&app.editor_font);
    let preview_box = container(
        column![
            text("LIVE EDITOR PREVIEW").size(10).color(TEXT_MUTED),
            Space::with_height(6),
            row![
                text("def ").size(13).font(preview_font).color(SYNTAX_KEYWORD),
                text("compute_norm").size(13).font(preview_font).color(SYNTAX_FUNCTION),
                text("(scale: float = 1.0) -> float:").size(13).font(preview_font).color(SYNTAX_PARAMETER),
            ],
            row![
                text("    \"\"\"Calculates vector norm with Ruff AST.\"\"\"").size(13).font(preview_font).color(SYNTAX_STRING),
            ],
            row![
                text("    return ").size(13).font(preview_font).color(SYNTAX_KEYWORD),
                text("scale * 42.0").size(13).font(preview_font).color(SYNTAX_NUMBER),
            ],
        ]
        .spacing(2)
        .padding(12),
    )
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(BG_EDITOR.into()),
        border: iced::border::rounded(6).color(BORDER_SUBTLE).width(1.0),
        ..Default::default()
    });

    let dialog_content = column![
        header,
        Space::with_height(12),
        sec1_title,
        sec1_chips,
        Space::with_height(8),
        sec2_title,
        sec2_chips,
        Space::with_height(8),
        sec3_title,
        sec3_chips,
        Space::with_height(14),
        preview_box,
    ]
    .spacing(6)
    .padding(20)
    .width(480);

    let modal = container(dialog_content)
        .style(|_| container::Style {
            background: Some(BG_CARD.into()),
            border: iced::border::rounded(8).color(BORDER_FOCUSED).width(1.0),
            ..Default::default()
        });

    // Dark backdrop filling the screen
    container(modal)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.6).into()),
            ..Default::default()
        })
        .into()
}

fn font_chip<'a>(
    label: &'static str,
    font_name: &'static str,
    active_font: &str,
    msg: EditorCommand,
) -> Element<'a, EditorCommand> {
    let is_selected = active_font == font_name;

    let mut chip_row = row![text(label).size(12).color(if is_selected {
        TEXT_PRIMARY
    } else {
        TEXT_MUTED
    })]
    .align_y(Alignment::Center);

    if is_selected {
        chip_row = chip_row.push(text("  ✓").size(11).color(BADGE_SUCCESS));
    }

    button(chip_row)
        .on_press(msg)
        .padding([6, 12])
    .style(move |_, status| {
        let border_color = if is_selected {
            BORDER_FOCUSED
        } else {
            BORDER_SUBTLE
        };
        let bg = match status {
            button::Status::Hovered => Color::from_rgb(0.20, 0.24, 0.32),
            _ if is_selected => Color::from_rgb(0.18, 0.22, 0.30),
            _ => Color::from_rgb(0.13, 0.15, 0.19),
        };

        button::Style {
            background: Some(bg.into()),
            border: iced::border::rounded(6).color(border_color).width(1.0),
            ..Default::default()
        }
    })
    .into()
}

fn empty_btn_style(_theme: &iced::Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: None,
        ..Default::default()
    }
}
