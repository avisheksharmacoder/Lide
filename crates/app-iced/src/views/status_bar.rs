//! Bottom status bar view: Cursor coordinates, Python venv, diagnostics, AI model indicator, and file state.

use iced::widget::{button, container, row, text, Space};
use iced::{Alignment, Element, Length};

use crate::icons::{icon_colored, Icon};
use crate::{AiConnectionStatus, EditorCommand, LideApp, SettingsTab};

pub fn render_status_bar(app: &LideApp) -> Element<'_, EditorCommand> {
    let theme = &app.current_theme;

    let status_str = if app.status_bar_text.is_empty() {
        "Ln 1, Col 1 | UTF-8 | Python"
    } else {
        app.status_bar_text.as_str()
    };

    let status_dot_color = match app.ai_status {
        AiConnectionStatus::Connected => theme.badge_success,
        AiConnectionStatus::Checking => theme.badge_warning,
        AiConnectionStatus::Error => theme.badge_error,
        AiConnectionStatus::Disconnected => theme.text_muted,
    };

    let cap_sz = app.ui_caption_size();

    let ai_model_btn = button(
        row![
            icon_colored(Icon::Cpu, theme.text_accent),
            text(&app.ai_model_name)
                .size(cap_sz)
                .line_height(iced::widget::text::LineHeight::Relative(1.2))
                .color(theme.text_accent),
            container(Space::with_width(6))
                .width(6)
                .height(6)
                .style(move |_| container::Style {
                    background: Some(status_dot_color.into()),
                    border: iced::border::rounded(3),
                    ..Default::default()
                }),
        ]
        .spacing(5)
        .align_y(Alignment::Center),
    )
    .on_press(EditorCommand::OpenSettingsDialog(SettingsTab::AiModels))
    .padding([2, 6])
    .style(move |_, status| {
        let bg = match status {
            button::Status::Hovered => Some(theme.bg_hover.into()),
            _ => None,
        };
        button::Style {
            background: bg,
            border: iced::border::rounded(4),
            ..Default::default()
        }
    });

    let is_fm_open = app.file_manager_open;
    let file_mgr_btn = button(
        row![
            icon_colored(
                if is_fm_open {
                    Icon::FolderOpen
                } else {
                    Icon::Folder
                },
                if is_fm_open {
                    theme.text_accent
                } else {
                    theme.text_primary
                },
            ),
        ]
        .align_y(Alignment::Center),
    )
    .on_press(EditorCommand::ToggleFileManager)
    .padding([2, 6])
    .style(move |_, status| {
        let bg = match status {
            button::Status::Hovered => Some(theme.bg_hover.into()),
            button::Status::Pressed => Some(theme.bg_hover.into()),
            _ => if is_fm_open {
                Some(theme.bg_hover.into())
            } else {
                None
            },
        };
        let border_color = if is_fm_open {
            theme.border_focused
        } else {
            iced::Color::TRANSPARENT
        };
        button::Style {
            background: bg,
            border: iced::border::rounded(4).color(border_color).width(1.0),
            ..Default::default()
        }
    });

    let bar = row![
        file_mgr_btn,
        Space::with_width(6),
        text(status_str)
            .size(cap_sz)
            .line_height(iced::widget::text::LineHeight::Relative(1.2))
            .color(theme.text_muted),
        Space::with_width(Length::Fill),
        ai_model_btn,
        Space::with_width(12),
        text("UTF-8")
            .size(cap_sz)
            .line_height(iced::widget::text::LineHeight::Relative(1.2))
            .color(theme.text_muted),
        Space::with_width(12),
        text("Python 3")
            .size(cap_sz)
            .line_height(iced::widget::text::LineHeight::Relative(1.2))
            .color(theme.text_accent),
    ]
    .spacing(4)
    .align_y(Alignment::Center)
    .padding([3, 10]);

    let bg_status = theme.bg_status_bar;
    let border_subtle = theme.border_subtle;

    container(bar)
        .width(Length::Fill)
        .height(Length::Shrink)
        .style(move |_| container::Style {
            background: Some(bg_status.into()),
            border: iced::border::color(border_subtle).width(1.0),
            ..Default::default()
        })
        .into()
}
