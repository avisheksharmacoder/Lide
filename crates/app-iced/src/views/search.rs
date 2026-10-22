//! Find-in-Files regex search bottom panel view.

use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Element, Length};

use crate::fonts::resolve_font_by_name;
use crate::icons::{icon_colored, Icon};
use crate::theme::ThemePalette;
use crate::{EditorCommand, LideApp};

pub fn render_search_panel(app: &LideApp) -> Element<'_, EditorCommand> {
    let font = resolve_font_by_name(&app.terminal_font);
    let theme = &app.current_theme;

    let query_input = text_input(
        "Regex pattern to search across workspace (e.g. def\\s+test_\\w+)...",
        &app.search_query,
    )
    .on_input(EditorCommand::SetSearchQuery)
    .on_submit(EditorCommand::ExecuteWorkspaceSearch { max_results: 100 })
    .padding([6, 10])
    .size(12)
    .font(font)
    .width(Length::Fill);

    let btn_search = button(
        row![
            icon_colored(Icon::Search, theme.text_primary),
            text("Search").size(12).color(theme.text_primary)
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .on_press(EditorCommand::ExecuteWorkspaceSearch { max_results: 100 })
    .padding([6, 12])
    .style(action_btn_style(theme));

    let btn_close = button(icon_colored(Icon::X, theme.text_muted))
        .on_press(EditorCommand::ToggleSearchPanel)
        .padding([6, 10])
        .style(close_btn_style);

    let header = row![
        text("FIND IN FILES").size(11).color(theme.text_accent),
        Space::with_width(12),
        query_input,
        btn_search,
        btn_close,
    ]
    .align_y(Alignment::Center)
    .spacing(8)
    .padding([6, 12]);

    let results_content: Element<'_, EditorCommand> = if let Some(ref err) = app.search_error {
        container(text(format!("Search Error: {err}")).size(12).color(theme.badge_error))
            .padding(12)
            .into()
    } else if app.search_results.is_empty() {
        container(text("No matches found or search not yet executed.").size(12).color(theme.text_muted))
            .padding(12)
            .into()
    } else {
        let items = app.search_results.iter().map(|hit| {
            let hit_clone = hit.clone();
            let path_str = hit.path.to_string_lossy().to_string();
            let line_num = hit.line_number;
            let preview = hit.line_text.trim();

            button(
                row![
                    text(format!("{}:{}", path_str, line_num))
                        .size(11.5)
                        .font(font)
                        .color(theme.text_accent),
                    Space::with_width(12),
                    text(preview).size(12).font(font).color(theme.text_primary),
                ]
                .spacing(6),
            )
            .on_press(EditorCommand::ClickSearchResult(hit_clone))
            .padding([3, 8])
            .width(Length::Fill)
            .style(hit_btn_style(theme))
            .into()
        });

        scrollable(column(items).spacing(2).padding(6)).into()
    };

    let bg_panel = theme.bg_panel;
    let border_subtle = theme.border_subtle;

    container(column![header, results_content])
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

fn close_btn_style(_theme: &iced::Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: None,
        ..Default::default()
    }
}

fn hit_btn_style(theme: &ThemePalette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    let bg_hover = theme.bg_hover;
    move |_, status| {
        let bg = match status {
            button::Status::Hovered => Some(bg_hover.into()),
            _ => None,
        };
        button::Style {
            background: bg,
            border: iced::border::rounded(4),
            ..Default::default()
        }
    }
}
