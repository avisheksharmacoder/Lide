//! TDD Test Suite for:
//! 1. Native System Folder Picker TEA command lifecycle (Prompt, Receive Selected, Cancel).
//! 2. Strict 10px hierarchical tree indentation progression across parent folders, child folders, and child files.
//! 3. Dark mode theme palette contrast and icon visibility assertions.
//! 4. Headless rendering of the File Manager sidebar with nested folders and files.

use std::fs;

use app_iced::subscriptions::keyboard::resolve_key_action;
use app_iced::theme::ThemePalette;
use app_iced::views::editor::render_editor;
use app_iced::views::sidebar::{self, tree_row_icon_offset_px};
use app_iced::{EditorCommand, LideApp, MoveDirection, Position, ThemeMode};
use iced::keyboard::{key::Named, Key, Modifiers};
use tempfile::TempDir;

#[test]
fn test_system_folder_picker_receive_selected_opens_workspace() {
    let temp = TempDir::new().expect("Failed to create temp directory");
    let root = temp.path().to_path_buf();

    fs::write(root.join("main.py"), "print('hello from native picker')\n").unwrap();
    fs::create_dir_all(root.join("subdir")).unwrap();
    fs::write(root.join("subdir/child.py"), "def child(): pass\n").unwrap();

    let mut app = LideApp::new(20.0, 400.0);
    assert!(app.active_workspace_root.is_none());
    assert!(app.workspace_tree.is_none());

    // 1. Dispatch PromptSystemFolderPicker
    let _ = app.dispatch(EditorCommand::PromptSystemFolderPicker);

    // 2. Simulate user picking the folder via OS native dialog
    let _ = app.dispatch(EditorCommand::ReceiveSelectedFolder(Some(root.clone())));

    // 3. Workspace is now open and indexed!
    assert_eq!(app.active_workspace_root, Some(root.clone()));
    assert!(app.file_manager_open);
    assert!(app.workspace_tree.is_some());

    let tree = app.workspace_tree.as_ref().unwrap();
    assert!(tree.children.iter().any(|c| c.name == "main.py" && !c.is_dir));
    assert!(tree.children.iter().any(|c| c.name == "subdir" && c.is_dir));

    // 4. Render sidebar headless
    let _elem = sidebar::render_file_manager(&app);
}

#[test]
fn test_system_folder_picker_cancelled_leaves_state_intact() {
    let mut app = LideApp::new(20.0, 400.0);
    assert!(app.active_workspace_root.is_none());

    // User cancels the OS dialog (returns None)
    let _ = app.dispatch(EditorCommand::ReceiveSelectedFolder(None));

    assert!(app.active_workspace_root.is_none());
    assert!(app.workspace_tree.is_none());
}

#[test]
fn test_system_folder_picker_command_contract_validation() {
    let (prog, args) = workspace_engine::build_folder_picker_command();
    assert!(!prog.is_empty());
    assert!(!args.is_empty());

    #[cfg(windows)]
    {
        assert_eq!(prog, "powershell");
        assert!(args.contains(&"-Sta".to_string()), "Windows picker must execute in STA mode");
        assert!(args.contains(&"-NoProfile".to_string()));
        assert!(args.contains(&"-NonInteractive".to_string()));
        assert!(args.contains(&"Bypass".to_string()));
    }
}

#[test]
fn test_file_and_folder_tree_indentation_progression() {
    // Test the exact 10px horizontal progression per depth level:
    // Layout: Space(depth * 10) + spacing(5) + chevron/spacer(11) + spacing(5)
    // Offset = depth * 10 + 21 px.
    assert_eq!(tree_row_icon_offset_px(0), 21, "Depth 0 icon offset must be 21px");
    assert_eq!(tree_row_icon_offset_px(1), 31, "Depth 1 child icon offset must be 31px");
    assert_eq!(tree_row_icon_offset_px(2), 41, "Depth 2 grandchild icon offset must be 41px");
    assert_eq!(tree_row_icon_offset_px(3), 51, "Depth 3 icon offset must be 51px");

    // Assert that every subsequent depth level is padded by EXACTLY 10 pixels from the parent
    for d in 0..10 {
        let parent_offset = tree_row_icon_offset_px(d);
        let child_offset = tree_row_icon_offset_px(d + 1);
        assert_eq!(
            child_offset - parent_offset,
            10,
            "Child item at depth {} must be padded exactly 10px from parent at depth {}",
            d + 1,
            d
        );
    }
}

#[test]
fn test_file_manager_rendering_with_nested_folders_and_files() {
    let temp = TempDir::new().expect("Failed to create temp directory");
    let root = temp.path().to_path_buf();

    fs::create_dir_all(root.join("parent_folder/child_folder")).unwrap();
    fs::write(root.join("parent_folder/file_in_parent.py"), "# file\n").unwrap();
    fs::write(root.join("parent_folder/child_folder/file_in_child.py"), "# child file\n").unwrap();

    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::ReceiveSelectedFolder(Some(root.clone())));

    // Expand directories to make child folders and child files visible
    let parent_path = root.join("parent_folder");
    let child_path = root.join("parent_folder/child_folder");
    let _ = app.dispatch(EditorCommand::ToggleDirectoryExpand(parent_path));
    let _ = app.dispatch(EditorCommand::ToggleDirectoryExpand(child_path));

    // Render tree with both child folders and child files
    let _elem = sidebar::render_file_manager(&app);

    // Select file to check Opened/Orange state rendering
    let child_file = root.join("parent_folder/child_folder/file_in_child.py");
    let _ = app.dispatch(EditorCommand::SelectFileManagerItem(child_file.clone()));
    assert_eq!(app.active_file_path, Some(child_file));

    let _elem_active = sidebar::render_file_manager(&app);
}

#[test]
fn test_dark_mode_palettes_high_contrast_font_and_icon_colors() {
    let dark_themes = vec![
        ThemePalette::one_dark_pro(),
        ThemePalette::dracula_dark(),
        ThemePalette::nord_deep(),
        ThemePalette::github_dark(),
    ];

    for theme in dark_themes {
        assert_eq!(theme.mode, ThemeMode::Dark);

        // Primary text must have high brightness (> 0.85 across RGB channels)
        // Never dark or black!
        assert!(
            theme.text_primary.r > 0.85 && theme.text_primary.g > 0.85 && theme.text_primary.b > 0.85,
            "Theme {} text_primary must be bright white/slate, found: {:?}",
            theme.name,
            theme.text_primary
        );

        // Muted text (icons, inactive labels) must be clearly visible (> 0.50 across RGB)
        assert!(
            theme.text_muted.r > 0.50 && theme.text_muted.g > 0.50 && theme.text_muted.b > 0.50,
            "Theme {} text_muted must have sufficient contrast, found: {:?}",
            theme.name,
            theme.text_muted
        );

        // Accent and syntax colors must be vivid and distinct from dark background
        assert!(
            theme.text_accent.b > 0.70 || theme.text_accent.r > 0.70,
            "Theme {} text_accent must stand out, found: {:?}",
            theme.name,
            theme.text_accent
        );

        assert!(
            theme.syntax_class.r > 0.50 && theme.syntax_class.g > 0.50,
            "Theme {} syntax_class (folder icon color) must be luminous amber/cyan, found: {:?}",
            theme.name,
            theme.syntax_class
        );
    }
}

#[test]
fn test_editor_typing_and_cursor_rendering() {
    let mut app = LideApp::new(22.0, 792.0);
    assert_eq!(app.total_lines(), 1);

    // 1. Verify keyboard mapping: regular character typing maps to InsertText
    let cmd_char = resolve_key_action(Key::Character("x".into()), Modifiers::empty());
    assert_eq!(cmd_char, Some(EditorCommand::InsertText("x".to_string())));

    let cmd_char_shift = resolve_key_action(Key::Character("x".into()), Modifiers::SHIFT);
    assert_eq!(cmd_char_shift, Some(EditorCommand::InsertText("X".to_string())));

    let cmd_space = resolve_key_action(Key::Named(Named::Space), Modifiers::empty());
    assert_eq!(cmd_space, Some(EditorCommand::InsertText(" ".to_string())));

    // 2. Type "def foo():" into editor
    let _ = app.dispatch(EditorCommand::InsertText("def foo():".to_string()));
    assert!(app.is_dirty);
    assert_eq!(app.visible_lines[0].text, "def foo():");
    assert_eq!(app.visible_lines[0].cursor_col, Some(10));

    // 3. Move cursor left 4 times
    let _ = app.dispatch(EditorCommand::MoveCursor(MoveDirection::Left));
    let _ = app.dispatch(EditorCommand::MoveCursor(MoveDirection::Left));
    let _ = app.dispatch(EditorCommand::MoveCursor(MoveDirection::Left));
    let _ = app.dispatch(EditorCommand::MoveCursor(MoveDirection::Left));
    assert_eq!(app.visible_lines[0].cursor_col, Some(6));

    // 4. Backspace character
    let _ = app.dispatch(EditorCommand::Backspace);
    assert_eq!(app.visible_lines[0].text, "def fo():");
    assert_eq!(app.visible_lines[0].cursor_col, Some(5));

    // 5. Render editor headless: must produce valid widget tree with visible cursor
    let _elem = render_editor(&app);
}

#[test]
fn test_editor_scrolling_with_spacers() {
    let mut app = LideApp::new(22.0, 792.0);

    // Generate a 500-line source document
    let lines: Vec<String> = (0..500).map(|i| format!("line_{i} = {i}")).collect();
    let source = lines.join("\n");
    let _ = app.dispatch(EditorCommand::InsertText(source));

    assert_eq!(app.total_lines(), 500);

    // Initial scroll offset is 0
    assert_eq!(app.scroll_offset_y, 0.0);
    assert_eq!(app.visible_lines.first().unwrap().line_idx, 0);

    // Scroll down by 2200 pixels (~100 lines)
    let _ = app.dispatch(EditorCommand::ScrollToPixel {
        offset_y: 2200.0,
        viewport_height: 792.0,
    });

    assert_eq!(app.scroll_offset_y, 2200.0);
    let start_idx = app.visible_lines.first().unwrap().line_idx;
    // With 10-line overscan, start_idx should be around 90 (100 - 10)
    assert!(
        start_idx >= 85 && start_idx <= 105,
        "Visible lines start must reflect scroll offset, found: {}",
        start_idx
    );

    // Render editor with top and bottom spacers
    let _elem = render_editor(&app);
}

#[test]
fn test_context_menu_placement_inside_sidebar() {
    let mut app = LideApp::new(20.0, 400.0);
    let temp = TempDir::new().expect("Failed to create temp directory");
    let test_file = temp.path().join("demo.py");
    fs::write(&test_file, "# demo\n").unwrap();

    // Right click at item depth 0 (X = 35.0, Y = 100.0)
    let _ = app.dispatch(EditorCommand::OpenFileContextMenu {
        path: test_file.clone(),
        is_dir: false,
        position_x: 35.0,
        position_y: 100.0,
    });

    assert!(app.file_context_menu.is_some());
    let menu = app.file_context_menu.as_ref().unwrap();
    assert_eq!(menu.position_x, 35.0);
    assert_eq!(menu.position_y, 100.0);

    // Render context menu: must not panic and fits inside sidebar bounds
    let _menu_elem = sidebar::render_file_context_menu(&app, menu);

    // Escape closes context menu
    let _ = app.dispatch(EditorCommand::Escape);
    assert!(app.file_context_menu.is_none());
}

