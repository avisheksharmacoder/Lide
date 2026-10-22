//! AI Assistant right split pane view for Ollama code generation and refactoring.

use ai_engine::AiTaskKind;
use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Alignment, Element, Length};

use crate::fonts::resolve_font_by_name;
use crate::icons::{icon_colored, Icon};
use crate::theme::ThemePalette;
use crate::{EditorCommand, LideApp};

pub fn render_ai_pane(app: &LideApp) -> Element<'_, EditorCommand> {
    let font = resolve_font_by_name(&app.editor_font);
    let theme = &app.current_theme;

    let header = row![
        text("AI ASSISTANT").size(11).color(theme.text_accent),
        Space::with_width(Length::Fill),
        text(&app.ai_model_name).size(10.5).color(theme.text_muted),
        Space::with_width(8),
        button(icon_colored(Icon::X, theme.text_muted))
            .on_press(EditorCommand::ToggleAiPane)
            .padding([2, 6])
            .style(empty_btn_style),
    ]
    .align_y(Alignment::Center)
    .padding([8, 12]);

    // Task triggers
    let btn_docstring = button(text("Docstring").size(11).color(theme.text_primary))
        .on_press(EditorCommand::TriggerAiTask(AiTaskKind::GenerateDocstring))
        .padding([4, 8])
        .style(action_btn_style(theme));

    let btn_explain = button(text("Complete").size(11).color(theme.text_primary))
        .on_press(EditorCommand::TriggerAiTask(AiTaskKind::InlineCompletion))
        .padding([4, 8])
        .style(action_btn_style(theme));

    let btn_refactor = button(text("Refactor").size(11).color(theme.text_primary))
        .on_press(EditorCommand::TriggerAiTask(AiTaskKind::Refactor {
            instruction: "Optimize and refactor function".to_string(),
        }))
        .padding([4, 8])
        .style(action_btn_style(theme));

    let task_bar = row![btn_docstring, btn_explain, btn_refactor]
        .spacing(6)
        .padding([4, 12]);

    // Enclosing context indicator
    let ctx_info = if let Some(ref ctx) = app.active_ai_context {
        text(format!(
            "Context: {} (Lines {}-{})",
            ctx.symbol_name,
            ctx.start_line + 1,
            ctx.end_line + 1
        ))
        .size(11)
        .color(theme.syntax_function)
    } else {
        text("Context: Whole file / Global").size(11).color(theme.text_muted)
    };

    let ctx_container = container(ctx_info).padding([4, 12]);

    let bg_card = theme.bg_card;
    let border_subtle = theme.border_subtle;

    // Stream text output
    let stream_box = container(
        scrollable(
            text(&app.ai_stream_text)
                .size(12)
                .font(font)
                .color(theme.text_primary),
        )
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .padding(10)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| container::Style {
        background: Some(bg_card.into()),
        border: iced::border::rounded(6).color(border_subtle).width(1.0),
        ..Default::default()
    });

    // Action buttons at bottom: Apply or Cancel
    let mut bottom_actions = Vec::new();

    if app.ai_clean_code.is_some() {
        bottom_actions.push(
            button(
                row![
                    icon_colored(Icon::Check, theme.badge_success),
                    text("Apply Suggestion to Code").size(12).color(theme.badge_success)
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .on_press(EditorCommand::ApplyAiSuggestion)
            .padding([6, 12])
            .style(action_btn_style(theme))
            .into(),
        );
    }

    if app.ai_is_streaming {
        bottom_actions.push(
            button(
                row![
                    icon_colored(Icon::Square, theme.badge_error),
                    text("Cancel Stream").size(12).color(theme.badge_error)
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .on_press(EditorCommand::CancelAiStream)
            .padding([6, 12])
            .style(action_btn_style(theme))
            .into(),
        );
    }

    let bottom_bar = row(bottom_actions).spacing(8).padding([8, 12]);
    let bg_sidebar = theme.bg_sidebar;

    container(column![header, task_bar, ctx_container, stream_box, bottom_bar])
        .width(320)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(bg_sidebar.into()),
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
