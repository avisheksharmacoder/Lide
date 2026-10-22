//! Virtualized code editor view: Gutter line numbers, diagnostics badges,
//! and dynamic theme syntax highlighted code spans.

use iced::widget::{column, container, mouse_area, row, scrollable, stack, text, Space};
use iced::{Alignment, Element, Length};
use lang_contract::DiagnosticSeverity;

use crate::fonts::resolve_font_by_name;
use crate::theme::ThemePalette;
use crate::{EditorCommand, LideApp, RenderedLine};

pub fn render_editor(app: &LideApp) -> Element<'_, EditorCommand> {
    let editor_font = resolve_font_by_name(&app.editor_font);
    let theme = &app.current_theme;

    let total_lines = app.buffer.len_lines();
    let line_height = app.line_height_px as f32;

    let start_line = app.visible_lines.first().map(|l| l.line_idx).unwrap_or(0);
    let end_line = app.visible_lines.last().map(|l| l.line_idx + 1).unwrap_or(0);

    let top_spacer_height = (start_line as f32) * line_height;
    let bottom_spacer_height = (total_lines.saturating_sub(end_line) as f32) * line_height;

    let mut content_elements = Vec::with_capacity(app.visible_lines.len() + 2);
    if top_spacer_height > 0.0 {
        content_elements.push(Space::with_height(top_spacer_height).into());
    }

    for line in &app.visible_lines {
        content_elements.push(render_editor_row(
            line,
            editor_font,
            app.line_height_px,
            app.editor_font_size,
            app.line_numbers_enabled,
            theme,
        ));
    }

    if bottom_spacer_height > 0.0 {
        let bottom_spacer = mouse_area(
            container(Space::with_height(bottom_spacer_height))
                .width(Length::Fill)
                .height(Length::Fixed(bottom_spacer_height)),
        )
        .on_press(EditorCommand::ClickEditorEmptySpace);
        content_elements.push(bottom_spacer.into());
    }

    let content = column(content_elements).width(Length::Fill);

    container(
        scrollable(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .on_scroll(|viewport| {
                EditorCommand::ScrollToPixel {
                    offset_y: viewport.absolute_offset().y as f64,
                    viewport_height: viewport.bounds().height as f64,
                }
            }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| container::Style {
        background: Some(theme.bg_editor.into()),
        ..Default::default()
    })
    .into()
}

fn render_editor_row<'a>(
    line: &'a RenderedLine,
    editor_font: iced::Font,
    line_height: f64,
    font_size: f32,
    show_line_numbers: bool,
    theme: &'a ThemePalette,
) -> Element<'a, EditorCommand> {
    let line_idx = line.line_idx;
    let is_active = line.is_active_line;

    // Diagnostic badge
    let (badge_char, badge_color) = match line.highest_diagnostic {
        Some(DiagnosticSeverity::Error) => ("●", theme.badge_error),
        Some(DiagnosticSeverity::Warning) => ("▲", theme.badge_warning),
        Some(DiagnosticSeverity::Information) | Some(DiagnosticSeverity::Hint) => {
            ("•", theme.badge_info)
        }
        None => (" ", iced::Color::TRANSPARENT),
    };

    let gutter_color = if is_active {
        theme.text_primary
    } else {
        theme.text_muted
    };

    let accent_color = theme.text_accent;

    // Gutter: Active indicator bar | Badge | Gutter Number
    let gutter_element: Element<'a, EditorCommand> = if show_line_numbers {
        row![
            container(Space::new(3, Length::Fill)).style(move |_| container::Style {
                background: if is_active {
                    Some(accent_color.into())
                } else {
                    None
                },
                ..Default::default()
            }),
            text(badge_char)
                .size(10)
                .line_height(iced::widget::text::LineHeight::Relative(1.2))
                .color(badge_color),
            text(format!("{:>5}", line.gutter_number))
                .size(12)
                .line_height(iced::widget::text::LineHeight::Relative(1.2))
                .font(editor_font)
                .color(gutter_color),
        ]
        .align_y(Alignment::Center)
        .spacing(4)
        .width(64)
        .into()
    } else {
        Space::with_width(8).into()
    };

    // Code Segments (pure text, positionally invariant)
    let code_elements = render_code_segments(line, editor_font, font_size, theme);
    let code_row: Element<'a, EditorCommand> = row(code_elements).spacing(0).into();

    // Caret overlay: stacked directly on top so text never shifts
    let code_display: Element<'a, EditorCommand> = if is_active && line.cursor_col.is_some() {
        let col = line.cursor_col.unwrap();
        let cursor_byte = line
            .text
            .char_indices()
            .nth(col)
            .map(|(idx, _)| idx)
            .unwrap_or(line.text.len());

        let prefix_text = &line.text[..cursor_byte.min(line.text.len())];

        let caret_height = (font_size * 1.25).min(line_height as f32 - 2.0).max(14.0);
        let caret_bar = container(Space::new(Length::Fixed(2.5), Length::Fixed(caret_height)))
            .width(Length::Fixed(2.5))
            .height(Length::Fixed(caret_height))
            .style(move |_| container::Style {
                background: Some(theme.text_accent.into()),
                ..Default::default()
            });

        let caret_overlay = row![
            text(prefix_text)
                .size(font_size)
                .line_height(iced::widget::text::LineHeight::Relative(1.2))
                .font(editor_font)
                .color(iced::Color::TRANSPARENT),
            caret_bar,
        ]
        .align_y(Alignment::Center)
        .spacing(0);

        stack(vec![
            container(code_row).width(Length::Fill).into(),
            container(caret_overlay).width(Length::Fill).into(),
        ])
        .width(Length::Fill)
        .into()
    } else {
        container(code_row).width(Length::Fill).into()
    };

    let row_bg = if is_active {
        theme.bg_active_row
    } else {
        theme.bg_editor
    };

    mouse_area(
        container(
            row![gutter_element, Space::with_width(8), code_display]
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .height(line_height as f32),
        )
        .width(Length::Fill)
        .height(line_height as f32)
        .padding(0)
        .style(move |_| container::Style {
            background: Some(row_bg.into()),
            ..Default::default()
        }),
    )
    .on_press(EditorCommand::ClickEditorLine { line_idx })
    .into()
}

pub fn render_code_segments<'a>(
    line: &'a RenderedLine,
    editor_font: iced::Font,
    font_size: f32,
    theme: &'a ThemePalette,
) -> Vec<Element<'a, EditorCommand>> {
    let mut code_elements: Vec<Element<'a, EditorCommand>> = Vec::new();

    // Case 1: Empty line
    if line.text.is_empty() {
        code_elements.push(
            text(" ")
                .size(font_size)
                .line_height(iced::widget::text::LineHeight::Relative(1.2))
                .font(editor_font)
                .color(theme.text_primary)
                .into(),
        );
        return code_elements;
    }

    // Case 2: Unstyled text
    if line.styled_segments.is_empty() {
        code_elements.push(
            text(&line.text)
                .size(font_size)
                .line_height(iced::widget::text::LineHeight::Relative(1.2))
                .font(editor_font)
                .color(theme.text_primary)
                .into(),
        );
        return code_elements;
    }

    // Case 3: Syntax highlighted segments
    for seg in &line.styled_segments {
        let start = seg.rel_byte_range.start;
        let end = seg.rel_byte_range.end;
        let color = theme.token_color(seg.token);

        let slice = line.text.get(start..end).unwrap_or("");
        if !slice.is_empty() {
            code_elements.push(
                text(slice)
                    .size(font_size)
                    .line_height(iced::widget::text::LineHeight::Relative(1.2))
                    .font(editor_font)
                    .color(color)
                    .into(),
            );
        }
    }

    code_elements
}
