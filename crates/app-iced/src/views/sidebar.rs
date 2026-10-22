//! Left sidebar view: Hierarchical Project File Manager and AST structure outline.

use iced::widget::{
    button, column, container, mouse_area, row, scrollable, text, text_input, Space,
};
use iced::{Alignment, Color, Element, Length};
use lang_contract::SymbolKind;

use crate::icons::{icon_sized_colored, Icon};
use crate::theme::ThemePalette;
use crate::{
    EditorCommand, FileContextMenuState, FileItemState, LideApp, NewItemPromptState,
    OpenFolderPromptState, SidebarTab,
};

/// Primary entry point to render the left-side File Manager sidebar panel.
pub fn render_file_manager(app: &LideApp) -> Element<'_, EditorCommand> {
    render_sidebar_panel(app)
}

/// Backwards-compatible sidebar renderer.
pub fn render_sidebar(app: &LideApp) -> Element<'_, EditorCommand> {
    if app.file_manager_open {
        render_sidebar_panel(app)
    } else {
        column![].into()
    }
}

/// Renders the complete left sidebar container with header, actions, and tree content.
fn render_sidebar_panel(app: &LideApp) -> Element<'_, EditorCommand> {
    let theme = &app.current_theme;
    let cap_sz = app.ui_caption_size();

    // 1. Header: Back button when viewing Structure, or Structure trigger + workspace actions when viewing Files
    let header_content: Element<'_, EditorCommand> = match app.active_sidebar_tab {
        SidebarTab::Structure => {
            let btn_back = button(
                row![
                    icon_sized_colored(Icon::ArrowLeft, 12, theme.text_primary),
                    text("Files")
                        .size(cap_sz)
                        .color(theme.text_primary),
                ]
                .spacing(5)
                .align_y(Alignment::Center),
            )
            .on_press(EditorCommand::SwitchSidebarTab(SidebarTab::ProjectFiles))
            .padding([3, 6])
            .style(action_icon_style(theme));

            let title = text("STRUCTURE")
                .size(cap_sz)
                .font(crate::fonts::FONT_JETBRAINS_MONO_BOLD)
                .color(theme.text_accent);

            row![
                btn_back,
                Space::with_width(8),
                title,
                Space::with_width(Length::Fill),
            ]
            .align_y(Alignment::Center)
            .into()
        }
        SidebarTab::ProjectFiles => {
            let btn_structure = button(
                text("STRUCTURE")
                    .size(cap_sz)
                    .color(theme.text_muted),
            )
            .on_press(EditorCommand::SwitchSidebarTab(SidebarTab::Structure))
            .padding([3, 8])
            .style(move |_, status| tab_button_style(false, status, theme));

            let mut actions = row![btn_structure, Space::with_width(Length::Fill)]
                .spacing(4)
                .align_y(Alignment::Center);

            // Quick action buttons for root workspace
            if let Some(root_path) = &app.active_workspace_root {
                let root_for_file = root_path.clone();
                let root_for_folder = root_path.clone();
                let btn_new_file = button(icon_sized_colored(Icon::Plus, 12, theme.text_primary))
                    .on_press(EditorCommand::TriggerCreateFilePrompt(root_for_file))
                    .padding([2, 4])
                    .style(action_icon_style(theme));

                let btn_new_folder = button(icon_sized_colored(Icon::FolderPlus, 12, theme.text_primary))
                    .on_press(EditorCommand::TriggerCreateFolderPrompt(root_for_folder))
                    .padding([2, 4])
                    .style(action_icon_style(theme));

                let btn_reselect_folder = button(icon_sized_colored(Icon::FolderOpen, 12, theme.text_primary))
                    .on_press(EditorCommand::PromptSystemFolderPicker)
                    .padding([2, 4])
                    .style(action_icon_style(theme));

                actions = actions
                    .push(btn_new_file)
                    .push(btn_new_folder)
                    .push(btn_reselect_folder);
            } else {
                let btn_open_folder = button(icon_sized_colored(Icon::FolderOpen, 12, theme.text_primary))
                    .on_press(EditorCommand::PromptSystemFolderPicker)
                    .padding([2, 4])
                    .style(action_icon_style(theme));

                actions = actions.push(btn_open_folder);
            }

            actions.into()
        }
    };

    let header = container(header_content)
        .padding([6, 8])
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(theme.bg_sidebar.into()),
            border: iced::border::color(theme.border_subtle).width(0.5),
            ..Default::default()
        });

    // 2. Content: File Manager Tree or AST Structure
    let content: Element<'_, EditorCommand> = match app.active_sidebar_tab {
        SidebarTab::ProjectFiles => render_file_tree_view(app),
        SidebarTab::Structure => render_structure_view(app),
    };

    let bg_sidebar = theme.bg_sidebar;
    let border_subtle = theme.border_subtle;

    container(column![header, content])
        .width(app.sidebar_width_px)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(bg_sidebar.into()),
            border: iced::border::color(border_subtle).width(1.0),
            ..Default::default()
        })
        .into()
}

/// Renders the hierarchical project files tree view with 10px indentation per nesting level.
fn render_file_tree_view(app: &LideApp) -> Element<'_, EditorCommand> {
    let theme = &app.current_theme;

    let Some(tree) = &app.workspace_tree else {
        if let Some(prompt) = &app.open_folder_prompt {
            return render_open_folder_prompt(prompt, theme);
        }

        let btn_select_folder = button(
            row![
                icon_sized_colored(Icon::FolderPlus, 13, theme.text_primary),
                text("Select Folder...")
                    .size(11.5)
                    .line_height(iced::widget::text::LineHeight::Relative(1.2))
                    .color(theme.text_primary),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .on_press(EditorCommand::PromptSystemFolderPicker)
        .padding([6, 14])
        .style(action_primary_button_style(theme));

        let btn_enter_path = button(
            row![
                icon_sized_colored(Icon::Folder, 11, theme.text_muted),
                text("Enter Path Manually...")
                    .size(10.5)
                    .line_height(iced::widget::text::LineHeight::Relative(1.2))
                    .color(theme.text_muted),
            ]
            .spacing(5)
            .align_y(Alignment::Center),
        )
        .on_press(EditorCommand::TriggerOpenFolderPrompt)
        .padding([3, 8])
        .style(action_icon_style(theme));

        return container(
            column![
                icon_sized_colored(Icon::Folder, 28, theme.syntax_class),
                Space::with_height(4),
                text("No Folder Opened")
                    .size(12.0)
                    .line_height(iced::widget::text::LineHeight::Relative(1.2))
                    .color(theme.text_primary),
                text("Select a project directory to start editing and creating files.")
                    .size(10.5)
                    .line_height(iced::widget::text::LineHeight::Relative(1.3))
                    .color(theme.text_muted),
                Space::with_height(6),
                btn_select_folder,
                Space::with_height(2),
                btn_enter_path,
                Space::with_height(6),
                text("Shortcut: Ctrl+B to toggle")
                    .size(10.0)
                    .color(theme.text_muted),
            ]
            .spacing(4)
            .padding(16),
        )
        .into();
    };

    let flat_items = tree.flatten_visible(&app.expanded_dirs);
    let mut tree_rows: Vec<Element<'_, EditorCommand>> = Vec::with_capacity(flat_items.len() + 2);

    // Root folder banner
    let root_name = app
        .active_workspace_root
        .as_ref()
        .and_then(|r| r.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or(&tree.name);

    let root_banner = row![
        icon_sized_colored(Icon::Folder, 13, theme.syntax_class),
        text(root_name.to_uppercase())
            .size(11)
            .line_height(iced::widget::text::LineHeight::Relative(1.2))
            .wrapping(iced::widget::text::Wrapping::None)
            .color(theme.text_accent),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .padding([4, 8]);

    tree_rows.push(root_banner.into());

    // Root-level inline create prompt if active
    if let Some(prompt) = &app.new_item_prompt {
        if let Some(root_path) = &app.active_workspace_root {
            if prompt.parent_dir == *root_path {
                tree_rows.push(render_inline_create_input(prompt, 0, theme));
            }
        }
    }

    // Render each visible tree item
    for (idx, item) in flat_items.into_iter().enumerate() {
        let item_y = (idx as f32) * 28.0 + 44.0;
        let indent_px = (item.depth as u16) * 10; // Exact 10px indentation per level

        if item.is_dir {
            let is_expanded = item.is_expanded;
            let dir_path = item.path.clone();
            let dir_path_for_ctx = item.path.clone();
            let item_depth = item.depth;
            let has_matching_prompt = app
                .new_item_prompt
                .as_ref()
                .map(|p| p.parent_dir == item.path)
                .unwrap_or(false);

            let chevron = icon_sized_colored(
                if is_expanded {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                },
                11,
                theme.text_muted,
            );

            let folder_ico = icon_sized_colored(
                if is_expanded {
                    Icon::FolderOpen
                } else {
                    Icon::Folder
                },
                13,
                theme.syntax_class,
            );

            let folder_content = row![
                Space::with_width(indent_px),
                chevron,
                folder_ico,
                text(item.name)
                    .size(11.5)
                    .line_height(iced::widget::text::LineHeight::Relative(1.2))
                    .wrapping(iced::widget::text::Wrapping::None)
                    .color(theme.text_primary),
            ]
            .spacing(5)
            .align_y(Alignment::Center);

            let btn = button(folder_content)
                .on_press(EditorCommand::ToggleDirectoryExpand(dir_path))
                .padding([3, 6])
                .width(Length::Fill)
                .style(item_button_style(theme));

            let row_element = mouse_area(btn)
                .on_right_press(EditorCommand::OpenFileContextMenu {
                    path: dir_path_for_ctx,
                    is_dir: true,
                    position_x: (item_depth as f32) * 10.0 + 35.0,
                    position_y: item_y,
                })
                .into();

            tree_rows.push(row_element);

            // If an inline creation prompt is active inside this folder, render it right below!
            if has_matching_prompt {
                if let Some(prompt) = &app.new_item_prompt {
                    let child_indent = (item_depth + 1) as u16 * 10;
                    tree_rows.push(render_inline_create_input(prompt, child_indent, theme));
                }
            }
        } else {
            // File Row with lifecycle state coloring (Orange on opened/modified, Green on saved)
            let file_path = item.path.clone();
            let file_path_for_ctx = item.path.clone();
            let is_active = app.active_file_path.as_ref() == Some(&item.path);

            // Determine status color
            let file_state = app
                .file_states
                .get(&item.path)
                .copied()
                .unwrap_or(if is_active {
                    if app.is_dirty {
                        FileItemState::Opened
                    } else {
                        FileItemState::Saved
                    }
                } else {
                    FileItemState::Normal
                });

            let (file_color, status_dot) = match file_state {
                FileItemState::Opened => (
                    theme.syntax_number, // ORANGE
                    Some(("●", theme.syntax_number)),
                ),
                FileItemState::Saved => (
                    theme.badge_success, // GREEN
                    Some(("●", theme.badge_success)),
                ),
                FileItemState::Normal => (
                    if is_active {
                        theme.text_accent
                    } else {
                        theme.text_primary
                    },
                    None,
                ),
            };

            let mut file_row = row![
                Space::with_width(indent_px),
                Space::with_width(11), // 11px spacer matching folder chevron width so child files strictly indent 10px below parent
                icon_sized_colored(Icon::FileCode, 13, file_color),
                text(item.name)
                    .size(11.5)
                    .line_height(iced::widget::text::LineHeight::Relative(1.2))
                    .wrapping(iced::widget::text::Wrapping::None)
                    .color(file_color),
                Space::with_width(Length::Fill),
            ]
            .spacing(5)
            .align_y(Alignment::Center);

            if let Some((dot_char, dot_color)) = status_dot {
                file_row = file_row.push(
                    text(dot_char)
                        .size(8)
                        .color(dot_color)
                        .line_height(iced::widget::text::LineHeight::Relative(1.0)),
                );
            }

            let btn = button(file_row)
                .on_press(EditorCommand::SelectFileManagerItem(file_path))
                .padding([3, 6])
                .width(Length::Fill)
                .style(file_row_button_style(is_active, file_state, theme));

            let row_element = mouse_area(btn)
                .on_right_press(EditorCommand::OpenFileContextMenu {
                    path: file_path_for_ctx,
                    is_dir: false,
                    position_x: (item.depth as f32) * 10.0 + 35.0,
                    position_y: item_y,
                })
                .into();

            tree_rows.push(row_element);
        }
    }

    scrollable(column(tree_rows).spacing(1).padding(4))
        .height(Length::Fill)
        .into()
}

/// Renders the inline file/folder name input row with submit & cancel controls.
fn render_inline_create_input<'a>(
    prompt: &NewItemPromptState,
    indent_px: u16,
    theme: &'a ThemePalette,
) -> Element<'a, EditorCommand> {
    let ico = if prompt.is_folder {
        Icon::FolderPlus
    } else {
        Icon::Plus
    };

    let placeholder_text = if prompt.is_folder {
        "folder name..."
    } else {
        "file name..."
    };

    let input_field = text_input(placeholder_text, &prompt.input_value)
        .size(11.5)
        .padding([2, 6])
        .on_input(EditorCommand::UpdateNewItemName)
        .on_submit(EditorCommand::ConfirmCreateItem)
        .width(Length::Fill)
        .style(move |_, _| text_input::Style {
            background: theme.bg_app.into(),
            border: iced::border::rounded(3).color(theme.border_focused).width(1.0),
            icon: theme.text_muted,
            placeholder: theme.text_muted,
            value: theme.text_primary,
            selection: theme.border_focused,
        });

    let btn_ok = button(icon_sized_colored(Icon::Check, 11, theme.badge_success))
        .on_press(EditorCommand::ConfirmCreateItem)
        .padding([2, 4])
        .style(action_icon_style(theme));

    let btn_cancel = button(icon_sized_colored(Icon::X, 11, theme.badge_error))
        .on_press(EditorCommand::CancelCreateItem)
        .padding([2, 4])
        .style(action_icon_style(theme));

    container(
        row![
            Space::with_width(indent_px),
            Space::with_width(11), // 11px spacer matching chevron column
            icon_sized_colored(ico, 12, theme.text_accent),
            input_field,
            btn_ok,
            btn_cancel,
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding([2, 4])
    .into()
}

/// Renders the floating context menu layer for a right-clicked file or folder.
pub fn render_file_context_menu<'a>(
    app: &'a LideApp,
    menu: &'a FileContextMenuState,
) -> Element<'a, EditorCommand> {
    let theme = &app.current_theme;

    let target_name = menu
        .target_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Item");

    let header_title = text(format!(" {target_name}"))
        .size(10.5)
        .color(theme.text_muted);

    let mut menu_items = column![header_title, Space::with_height(3)].spacing(2);

    if menu.is_dir {
        let parent_for_file = menu.target_path.clone();
        let parent_for_folder = menu.target_path.clone();
        let path_for_exp = menu.target_path.clone();
        let path_for_del = menu.target_path.clone();

        menu_items = menu_items
            .push(context_menu_item(
                Icon::Plus,
                "New File...",
                EditorCommand::TriggerCreateFilePrompt(parent_for_file),
                false,
                theme,
            ))
            .push(context_menu_item(
                Icon::FolderPlus,
                "New Folder...",
                EditorCommand::TriggerCreateFolderPrompt(parent_for_folder),
                false,
                theme,
            ))
            .push(context_menu_item(
                Icon::ExternalLink,
                "Reveal in Explorer",
                EditorCommand::RevealInExplorer(path_for_exp),
                false,
                theme,
            ))
            .push(context_menu_item(
                Icon::Trash2,
                "Delete Folder",
                EditorCommand::DeleteWorkspaceEntry(path_for_del),
                true,
                theme,
            ));
    } else {
        let path_for_open = menu.target_path.clone();
        let path_for_exp = menu.target_path.clone();
        let path_for_del = menu.target_path.clone();

        menu_items = menu_items
            .push(context_menu_item(
                Icon::FileCode,
                "Open File",
                EditorCommand::SelectFileManagerItem(path_for_open),
                false,
                theme,
            ))
            .push(context_menu_item(
                Icon::ExternalLink,
                "Reveal in Explorer",
                EditorCommand::RevealInExplorer(path_for_exp),
                false,
                theme,
            ))
            .push(context_menu_item(
                Icon::Trash2,
                "Delete File",
                EditorCommand::DeleteWorkspaceEntry(path_for_del),
                true,
                theme,
            ));
    }

    let menu_card = container(menu_items)
        .width(180)
        .padding(6)
        .style(move |_| container::Style {
            background: Some(theme.bg_card.into()),
            border: iced::border::rounded(6).color(theme.border_subtle).width(1.0),
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..Default::default()
        });

    let clamped_y = (menu.position_y - 6.0).max(36.0);
    let clamped_x = menu.position_x.clamp(16.0, 55.0);

    // Full screen backdrop catching outside clicks
    mouse_area(
        container(
            column![
                Space::with_height(clamped_y),
                row![Space::with_width(clamped_x), menu_card,],
            ]
        )
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .on_press(EditorCommand::CloseFileContextMenu)
    .into()
}

fn context_menu_item<'a>(
    ico: Icon,
    label: &'a str,
    cmd: EditorCommand,
    is_danger: bool,
    theme: &'a ThemePalette,
) -> Element<'a, EditorCommand> {
    let text_color = if is_danger {
        theme.badge_error
    } else {
        theme.text_primary
    };

    button(
        row![
            icon_sized_colored(ico, 12, text_color),
            text(label).size(11.5).color(text_color),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .on_press(cmd)
    .padding([4, 8])
    .width(Length::Fill)
    .style(move |_, status| {
        let bg = match status {
            button::Status::Hovered => Some(theme.bg_hover.into()),
            button::Status::Pressed => Some(theme.bg_hover.into()),
            _ => None,
        };
        button::Style {
            background: bg,
            border: iced::border::rounded(4),
            ..Default::default()
        }
    })
    .into()
}

/// AST Structure view (functions, classes, methods).
fn render_structure_view(app: &LideApp) -> Element<'_, EditorCommand> {
    let theme = &app.current_theme;

    if app.outline_items.is_empty() {
        container(
            text("No Python symbols found.\nEdit code to parse AST outline.")
                .size(11.5)
                .color(theme.text_muted),
        )
        .padding(16)
        .into()
    } else {
        let items = app.outline_items.iter().map(|item| {
            let indent = (item.depth * 14) as u16;
            let (badge, badge_color) = match item.kind {
                SymbolKind::Class => ("C", theme.syntax_class),
                SymbolKind::Function => ("f", theme.syntax_function),
                SymbolKind::Method => ("m", theme.syntax_function),
                SymbolKind::AsyncFunction => ("a", theme.syntax_function),
                SymbolKind::Variable => ("v", theme.syntax_number),
                SymbolKind::Constant => ("c", theme.syntax_number),
                SymbolKind::Module => ("M", theme.syntax_keyword),
                SymbolKind::CdefFunction => ("c", theme.syntax_decorator),
                SymbolKind::Struct => ("S", theme.syntax_class),
            };

            let start_line = item.start_line;

            button(
                row![
                    Space::with_width(indent),
                    text(badge).size(10.5).color(badge_color),
                    Space::with_width(6),
                    text(&item.name)
                        .size(11.5)
                        .wrapping(iced::widget::text::Wrapping::None)
                        .color(theme.text_primary),
                ]
                .align_y(Alignment::Center),
            )
            .on_press(EditorCommand::ClickStructureSymbol { start_line })
            .padding([3, 6])
            .width(Length::Fill)
            .style(item_button_style(theme))
            .into()
        });

        scrollable(column(items).spacing(2).padding(6)).into()
    }
}

fn tab_button_style(
    is_active: bool,
    status: button::Status,
    theme: &ThemePalette,
) -> button::Style {
    let border_color = if is_active {
        theme.border_focused
    } else {
        Color::TRANSPARENT
    };

    let bg = match status {
        button::Status::Hovered if !is_active => Some(theme.bg_hover.into()),
        _ => None,
    };

    button::Style {
        background: bg,
        border: iced::border::rounded(4).color(border_color).width(1.0),
        ..Default::default()
    }
}

fn item_button_style(theme: &ThemePalette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    let bg_hover = theme.bg_hover;
    move |_, status| {
        let bg = match status {
            button::Status::Hovered => Some(bg_hover.into()),
            button::Status::Pressed => Some(bg_hover.into()),
            _ => None,
        };

        button::Style {
            background: bg,
            border: iced::border::rounded(4),
            ..Default::default()
        }
    }
}

fn file_row_button_style(
    is_active: bool,
    state: FileItemState,
    theme: &ThemePalette,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    let bg_hover = theme.bg_hover;
    let border_color = match state {
        FileItemState::Opened => theme.syntax_number, // Orange
        FileItemState::Saved => theme.badge_success,  // Green
        FileItemState::Normal => theme.border_focused,
    };

    move |_, status| {
        let mut bg = match status {
            button::Status::Hovered => Some(bg_hover.into()),
            button::Status::Pressed => Some(bg_hover.into()),
            _ => None,
        };

        if is_active && bg.is_none() {
            bg = Some(bg_hover.into());
        }

        button::Style {
            background: bg,
            border: iced::border::rounded(4)
                .color(if is_active { border_color } else { Color::TRANSPARENT })
                .width(if is_active { 1.0 } else { 0.0 }),
            ..Default::default()
        }
    }
}

fn action_icon_style(theme: &ThemePalette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    let bg_hover = theme.bg_hover;
    move |_, status| {
        let bg = match status {
            button::Status::Hovered => Some(bg_hover.into()),
            button::Status::Pressed => Some(bg_hover.into()),
            _ => None,
        };
        button::Style {
            background: bg,
            border: iced::border::rounded(3),
            ..Default::default()
        }
    }
}

/// Renders the interactive prompt to add or open a workspace directory.
fn render_open_folder_prompt<'a>(
    prompt: &'a OpenFolderPromptState,
    theme: &'a ThemePalette,
) -> Element<'a, EditorCommand> {
    let title = row![
        icon_sized_colored(Icon::FolderPlus, 13, theme.syntax_class),
        text("Open or Add Folder")
            .size(11.5)
            .color(theme.text_primary),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let input_field = text_input("Path to folder (e.g. C:\\my_project or .)", &prompt.input_value)
        .size(11.0)
        .padding([4, 6])
        .on_input(EditorCommand::UpdateOpenFolderPath)
        .on_submit(EditorCommand::ConfirmOpenFolder)
        .width(Length::Fill)
        .style(move |_, _| text_input::Style {
            background: theme.bg_app.into(),
            border: iced::border::rounded(4).color(theme.border_focused).width(1.0),
            icon: theme.text_muted,
            placeholder: theme.text_muted,
            value: theme.text_primary,
            selection: theme.border_focused,
        });

    let btn_open = button(
        row![
            icon_sized_colored(Icon::Check, 11, theme.text_primary),
            text("Open")
                .size(11.0)
                .color(theme.text_primary),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .on_press(EditorCommand::ConfirmOpenFolder)
    .padding([3, 8])
    .style(action_primary_button_style(theme));

    let btn_cancel = button(
        row![
            icon_sized_colored(Icon::X, 11, theme.text_muted),
            text("Cancel")
                .size(11.0)
                .color(theme.text_muted),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .on_press(EditorCommand::CancelOpenFolderPrompt)
    .padding([3, 8])
    .style(action_secondary_button_style(theme));

    let btn_curr_dir = button(
        text("Use . (Current Dir)")
            .size(10.0)
            .color(theme.text_accent),
    )
    .on_press(EditorCommand::UpdateOpenFolderPath(".".to_string()))
    .padding([2, 4])
    .style(move |_, _| button::Style::default());

    let mut content = column![
        title,
        Space::with_height(2),
        text("Enter a folder path. If it does not exist, it will be created automatically.")
            .size(10.0)
            .line_height(iced::widget::text::LineHeight::Relative(1.3))
            .color(theme.text_muted),
        Space::with_height(4),
        input_field,
    ]
    .spacing(4);

    if let Some(err) = &prompt.error_message {
        content = content.push(
            text(err)
                .size(10.0)
                .color(theme.badge_error),
        );
    }

    content = content.push(
        row![btn_open, btn_cancel, Space::with_width(Length::Fill), btn_curr_dir]
            .spacing(6)
            .align_y(Alignment::Center),
    );

    container(content)
        .padding(14)
        .width(Length::Fill)
        .into()
}

fn action_primary_button_style(theme: &ThemePalette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    let bg_hover = theme.bg_hover;
    let border_focused = theme.border_focused;
    let bg_card = theme.bg_card;
    move |_, status| {
        let bg = match status {
            button::Status::Hovered => Some(bg_hover.into()),
            button::Status::Pressed => Some(bg_hover.into()),
            _ => Some(bg_card.into()),
        };
        button::Style {
            background: bg,
            border: iced::border::rounded(4).color(border_focused).width(1.0),
            ..Default::default()
        }
    }
}

fn action_secondary_button_style(theme: &ThemePalette) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    let bg_hover = theme.bg_hover;
    let border_subtle = theme.border_subtle;
    move |_, status| {
        let bg = match status {
            button::Status::Hovered => Some(bg_hover.into()),
            button::Status::Pressed => Some(bg_hover.into()),
            _ => None,
        };
        button::Style {
            background: bg,
            border: iced::border::rounded(4).color(border_subtle).width(1.0),
            ..Default::default()
        }
    }
}

/// Helper returning the horizontal offset in pixels for a tree row icon at `depth`.
/// Layout per row: Space(depth * 10) + spacing(5) + chevron/spacer(11) + spacing(5)
/// Total offset before item icon = depth * 10 + 21 pixels.
pub fn tree_row_icon_offset_px(depth: usize) -> u16 {
    (depth as u16) * 10 + 11 + 5 + 5
}

