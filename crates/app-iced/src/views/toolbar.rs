//! Top toolbar view with menus, title, quick-action buttons, and floating dropdowns.

use iced::widget::{button, column, container, mouse_area, row, text, Space};
use iced::{Alignment, Color, Element, Length};

use crate::icons::{icon_colored, Icon};
use crate::theme::ThemePalette;
use crate::{ActiveMenu, EditorCommand, LideApp};

pub fn render_toolbar(app: &LideApp) -> Element<'_, EditorCommand> {
    let theme = &app.current_theme;
    let ui_sz = app.ui_font_size();

    let brand = text("LIDE")
        .size(app.ui_title_size())
        .line_height(iced::widget::text::LineHeight::Relative(1.2))
        .font(crate::fonts::FONT_JETBRAINS_MONO_BOLD)
        .color(theme.text_accent);

    let files_btn = button(
        row![
            icon_colored(Icon::Folder, theme.text_primary),
            text("Files")
                .size(ui_sz)
                .line_height(iced::widget::text::LineHeight::Relative(1.2))
                .color(theme.text_primary),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .on_press(EditorCommand::ToggleMenu(ActiveMenu::File))
    .padding([4, 10])
    .style(container_button_style(theme));

    let settings_btn = button(
        row![
            icon_colored(Icon::Settings, theme.text_primary),
            text("Settings")
                .size(ui_sz)
                .line_height(iced::widget::text::LineHeight::Relative(1.2))
                .color(theme.text_primary),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .on_press(EditorCommand::ToggleSettingsDialog)
    .padding([4, 10])
    .style(container_button_style(theme));

    let btn_pytest = button(
        row![
            icon_colored(Icon::FlaskConical, theme.text_primary),
            text("Tests")
                .size(ui_sz)
                .line_height(iced::widget::text::LineHeight::Relative(1.2))
                .color(theme.text_primary)
        ]
        .spacing(5)
        .align_y(Alignment::Center),
    )
    .on_press(EditorCommand::TogglePytestPanel)
    .padding([4, 8])
    .style(action_button_style(theme));

    let btn_search = button(
        row![
            icon_colored(Icon::Search, theme.text_primary),
            text("Find")
                .size(ui_sz)
                .line_height(iced::widget::text::LineHeight::Relative(1.2))
                .color(theme.text_primary)
        ]
        .spacing(5)
        .align_y(Alignment::Center),
    )
    .on_press(EditorCommand::ToggleSearchPanel)
    .padding([4, 8])
    .style(action_button_style(theme));

    let btn_ai = button(
        row![
            icon_colored(Icon::Sparkles, theme.text_accent),
            text("AI Assistant")
                .size(ui_sz)
                .line_height(iced::widget::text::LineHeight::Relative(1.2))
                .color(theme.text_accent)
        ]
        .spacing(5)
        .align_y(Alignment::Center),
    )
    .on_press(EditorCommand::ToggleAiPane)
    .padding([4, 10])
    .style(ai_button_style(theme));

    let bar = row![
        brand,
        Space::with_width(12),
        files_btn,
        settings_btn,
        Space::with_width(Length::Fill),
        btn_pytest,
        btn_search,
        btn_ai,
    ]
    .align_y(Alignment::Center)
    .padding([6, 12])
    .spacing(6);

    let bg_toolbar = theme.bg_toolbar;
    let border_subtle = theme.border_subtle;

    container(bar)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(bg_toolbar.into()),
            border: iced::border::color(border_subtle).width(1.0),
            ..Default::default()
        })
        .into()
}

/// Floating dropdown menu for "Files" anchored over the editor without pushing UI down.
pub fn render_file_dropdown<'a>(app: &'a LideApp) -> Element<'a, EditorCommand> {
    let theme = &app.current_theme;

    let menu = container(
        column![
            menu_item(Icon::Plus, "New Python File", "Ctrl+N", EditorCommand::NewDocument, theme),
            menu_item(Icon::Folder, "Toggle File Manager", "Ctrl+B", EditorCommand::ToggleFileManager, theme),
            menu_item(Icon::FolderPlus, "Open Folder...", "Ctrl+O", EditorCommand::TriggerOpenFolderPrompt, theme),
            menu_item(Icon::Save, "Save & Format", "Ctrl+S", EditorCommand::SaveActiveFile, theme),
            menu_item(Icon::Zap, "Format Code", "Ctrl+Alt+F", EditorCommand::FormatDocument, theme),
            menu_item(Icon::Settings, "Settings...", "Ctrl+,", EditorCommand::ToggleSettingsDialog, theme),
            menu_item(Icon::X, "Close Menu", "Esc", EditorCommand::CloseMenu, theme),
        ]
        .spacing(2)
        .padding(6),
    )
    .style(move |_| container::Style {
        background: Some(theme.bg_card.into()),
        border: iced::border::rounded(6).color(theme.border_focused).width(1.0),
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        ..Default::default()
    });

    mouse_area(
        container(
            column![
                Space::with_height(34),
                row![
                    Space::with_width(55),
                    menu,
                ]
            ]
        )
        .width(Length::Fill)
        .height(Length::Fill)
    )
    .on_press(EditorCommand::CloseMenu)
    .into()
}

fn menu_item<'a>(
    icon_type: Icon,
    label: &'static str,
    shortcut: &'static str,
    msg: EditorCommand,
    theme: &'a ThemePalette,
) -> Element<'a, EditorCommand> {
    button(
        row![
            icon_colored(icon_type, theme.text_primary),
            text(label)
                .size(12)
                .line_height(iced::widget::text::LineHeight::Relative(1.2))
                .color(theme.text_primary),
            Space::with_width(Length::Fill),
            text(shortcut)
                .size(10.5)
                .line_height(iced::widget::text::LineHeight::Relative(1.2))
                .color(theme.text_muted),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .width(200)
    )
    .on_press(msg)
    .padding([5, 8])
    .style(container_button_style(theme))
    .into()
}

fn container_button_style(theme: &ThemePalette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    let bg_card = theme.bg_card;
    let text_primary = theme.text_primary;
    move |_, status| match status {
        button::Status::Hovered => button::Style {
            background: Some(bg_card.into()),
            text_color: text_primary,
            border: iced::border::rounded(4),
            ..Default::default()
        },
        _ => button::Style {
            background: None,
            text_color: text_primary,
            ..Default::default()
        },
    }
}

fn action_button_style(theme: &ThemePalette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    let bg_normal = theme.bg_card;
    let bg_hover = theme.bg_hover;
    let border_color = theme.border_subtle;
    let text_primary = theme.text_primary;

    move |_, status| {
        let bg = match status {
            button::Status::Hovered => bg_hover,
            _ => bg_normal,
        };

        button::Style {
            background: Some(bg.into()),
            text_color: text_primary,
            border: iced::border::rounded(4).color(border_color).width(1.0),
            ..Default::default()
        }
    }
}

fn ai_button_style(theme: &ThemePalette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    let bg_normal = theme.bg_card;
    let bg_hover = theme.bg_hover;
    let text_accent = theme.text_accent;
    let border_focused = theme.border_focused;

    move |_, status| {
        let bg = match status {
            button::Status::Hovered => bg_hover,
            _ => bg_normal,
        };

        button::Style {
            background: Some(bg.into()),
            text_color: text_accent,
            border: iced::border::rounded(4).color(border_focused).width(1.0),
            ..Default::default()
        }
    }
}
