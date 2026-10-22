//! Pytest runner bottom drawer view: Live test stream output & clickable failures.

use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Alignment, Color, Element, Length};

use crate::fonts::resolve_font_by_name;
use crate::icons::{icon_colored, Icon};
use crate::theme::ThemePalette;
use crate::{EditorCommand, LideApp};

pub fn render_pytest_panel(app: &LideApp) -> Element<'_, EditorCommand> {
    let font = resolve_font_by_name(&app.terminal_font);
    let theme = &app.current_theme;

    let summary_text = app
        .pytest_summary
        .as_deref()
        .unwrap_or("Ready to run pytest");

    let summary_color = if app.pytest_is_running {
        theme.badge_info
    } else if !app.pytest_failures.is_empty() {
        theme.badge_error
    } else {
        theme.badge_success
    };

    let summary_badge = text(summary_text).size(12).color(summary_color);

    let btn_run_all = button(
        row![
            icon_colored(Icon::Play, theme.badge_success),
            text("Run All").size(11.5).color(theme.text_primary)
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .on_press(EditorCommand::TriggerPytestRun { target_file: None })
    .padding([4, 8])
    .style(action_btn_style(theme));

    let btn_run_file = button(
        row![
            icon_colored(Icon::FileCode, theme.text_primary),
            text("Run File").size(11.5).color(theme.text_primary)
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .on_press(EditorCommand::TriggerPytestRun {
        target_file: app.active_file_path.clone(),
    })
    .padding([4, 8])
    .style(action_btn_style(theme));

    let btn_cancel: Element<'_, EditorCommand> = if app.pytest_is_running {
        button(
            row![
                icon_colored(Icon::Square, theme.badge_error),
                text("Cancel").size(11.5).color(theme.badge_error)
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        )
        .on_press(EditorCommand::CancelPytestRun)
        .padding([4, 8])
        .style(action_btn_style(theme))
        .into()
    } else {
        Space::with_width(0).into()
    };

    let btn_close = button(icon_colored(Icon::X, theme.text_muted))
        .on_press(EditorCommand::TogglePytestPanel)
        .padding([4, 8])
        .style(empty_btn_style);

    let header = row![
        text("PYTEST").size(11).color(theme.text_accent),
        Space::with_width(12),
        summary_badge,
        Space::with_width(Length::Fill),
        btn_run_all,
        btn_run_file,
        btn_cancel,
        btn_close,
    ]
    .align_y(Alignment::Center)
    .spacing(6)
    .padding([6, 12]);

    // Output console & Failures
    let mut logs: Vec<Element<'_, EditorCommand>> = Vec::new();

    // Render Clickable Failures first
    for failure in &app.pytest_failures {
        let f_clone = failure.clone();
        let path_str = failure.path.to_string_lossy().to_string();
        let line_num = failure.line_number;
        let test_name = failure.test_name.clone();

        logs.push(
            button(
                row![
                    text("✖").size(12).color(theme.badge_error),
                    Space::with_width(8),
                    text(format!("{path_str}:{line_num}"))
                        .size(11.5)
                        .font(font)
                        .color(theme.text_accent),
                    Space::with_width(8),
                    text(test_name).size(12).font(font).color(theme.badge_error),
                ]
                .align_y(Alignment::Center),
            )
            .on_press(EditorCommand::ClickPytestFailure(f_clone))
            .padding([2, 6])
            .width(Length::Fill)
            .style(failure_btn_style(theme))
            .into(),
        );
    }

    // Render stream stdout lines
    for line in &app.pytest_output_lines {
        logs.push(
            text(line)
                .size(11.5)
                .font(font)
                .color(theme.text_muted)
                .into(),
        );
    }

    let logs_scroll = scrollable(column(logs).spacing(2).padding(8))
        .width(Length::Fill)
        .height(Length::Fill);

    let bg_panel = theme.bg_panel;
    let border_subtle = theme.border_subtle;

    container(column![header, logs_scroll])
        .height(180)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(bg_panel.into()),
            border: iced::border::color(border_subtle).width(1.0),
            ..Default::default()
        })
        .into()
}

fn action_btn_style(theme: &ThemePalette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    let bg_normal = theme.bg_card;
    let bg_hover = theme.bg_hover;
    let border_color = theme.border_subtle;

    move |_, status| {
        let bg = match status {
            button::Status::Hovered => bg_hover,
            _ => bg_normal,
        };
        button::Style {
            background: Some(bg.into()),
            border: iced::border::rounded(4).color(border_color).width(1.0),
            ..Default::default()
        }
    }
}

fn empty_btn_style(_theme: &iced::Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: None,
        ..Default::default()
    }
}

fn failure_btn_style(theme: &ThemePalette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    let badge_error = theme.badge_error;
    move |_, status| {
        let bg = match status {
            button::Status::Hovered => Some(Color::from_rgba(0.9, 0.2, 0.2, 0.25).into()),
            _ => Some(Color::from_rgba(0.8, 0.2, 0.2, 0.15).into()),
        };
        button::Style {
            background: bg,
            border: iced::border::rounded(4).color(badge_error).width(1.0),
            ..Default::default()
        }
    }
}
