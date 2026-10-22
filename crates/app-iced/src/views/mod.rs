//! Modular view components for the Lide IDE shell in Iced.

pub mod ai_pane;
pub mod editor;
pub mod font_dialog;
pub mod pytest;
pub mod search;
pub mod settings_dialog;
pub mod sidebar;
pub mod status_bar;
pub mod toolbar;

use iced::widget::{column, container, mouse_area, row, stack, Space};
use iced::{Element, Length};

use crate::{ActiveMenu, EditorCommand, LideApp};

/// Main application view renderer. Assembles the toolbar, central editor,
/// bottom drawers (search/pytest), right assistant pane, status bar, and modal dialogs.
pub fn render_app(app: &LideApp) -> Element<'_, EditorCommand> {
    let theme = &app.current_theme;

    // 1. Center workspace: Left File Manager (if open) | Editor Area | AI Assistant
    let mut center_items: Vec<Element<'_, EditorCommand>> = Vec::new();

    if app.file_manager_open {
        let splitter = mouse_area(
            container(Space::new(4, Length::Fill)).style(move |_| container::Style {
                background: if app.is_resizing_sidebar {
                    Some(theme.border_focused.into())
                } else {
                    Some(theme.border_subtle.into())
                },
                ..Default::default()
            }),
        )
        .on_press(EditorCommand::StartResizeSidebar);

        center_items.push(sidebar::render_file_manager(app));
        center_items.push(splitter.into());
    }

    let editor_column = column![
        editor::render_editor(app),
        if app.search_panel_open {
            search::render_search_panel(app)
        } else if app.pytest_panel_open {
            pytest::render_pytest_panel(app)
        } else {
            column![].into()
        }
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    center_items.push(editor_column.into());

    if app.ai_pane_open {
        center_items.push(ai_pane::render_ai_pane(app));
    }

    let center_area = row(center_items)
        .width(Length::Fill)
        .height(Length::Fill);

    // 2. Full main shell: Toolbar -> Center Area -> Status Bar
    let main_shell = iced::widget::container(column![
        toolbar::render_toolbar(app),
        center_area,
        status_bar::render_status_bar(app),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| iced::widget::container::Style {
        background: Some(theme.bg_app.into()),
        ..Default::default()
    });

    // 3. Stack floating layers (dropdown menus, file context menu, settings dialog) on top without shifting main_shell
    let mut layers: Vec<Element<'_, EditorCommand>> = vec![main_shell.into()];

    if app.active_menu == ActiveMenu::File {
        layers.push(toolbar::render_file_dropdown(app));
    }

    if let Some(ctx_menu) = &app.file_context_menu {
        layers.push(sidebar::render_file_context_menu(app, ctx_menu));
    }

    if app.settings_dialog_open || app.font_dialog_open {
        layers.push(settings_dialog::render_settings_dialog(app));
    }

    stack(layers)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
