use std::fs;
use std::path::PathBuf;

use app_iced::views::{render_app, sidebar, status_bar};
use app_iced::{EditorCommand, FileContextMenuState, FileItemState, LideApp};
use tempfile::TempDir;

/// Helper: Creates a mock directory tree for testing UI rendering.
fn create_test_workspace() -> TempDir {
    let temp = TempDir::new().expect("Failed to create temp workspace");
    let root = temp.path();

    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join(".gitignore"), "build/\n").unwrap();

    fs::create_dir_all(root.join("src/core")).unwrap();
    fs::write(
        root.join("src/core/math_ops.py"),
        "def add(a: int, b: int) -> int:\n    return a + b\n",
    )
    .unwrap();
    fs::write(
        root.join("src/main.py"),
        "def main():\n    print('Lide IDE')\n",
    )
    .unwrap();
    fs::write(root.join("README.md"), "# Mock Workspace\n").unwrap();

    temp
}

#[test]
fn test_render_file_manager_closed_vs_open() {
    let mut app = LideApp::new(20.0, 400.0);
    assert!(!app.file_manager_open);

    // When closed, render_sidebar produces an empty column
    let _closed_elem = sidebar::render_sidebar(&app);

    // When toggled open, render_file_manager produces full sidebar
    app.file_manager_open = true;
    let _open_elem = sidebar::render_file_manager(&app);
}

#[test]
fn test_render_file_manager_tree_hierarchy_with_workspace() {
    let temp = create_test_workspace();
    let root = temp.path().to_path_buf();
    let mut app = LideApp::new(20.0, 400.0);

    let _ = app.dispatch(EditorCommand::OpenWorkspace(root.clone()));
    assert!(app.file_manager_open);
    assert!(app.workspace_tree.is_some());

    // 1. Initial render with collapsed folders
    let _elem1 = sidebar::render_file_manager(&app);

    // 2. Expand "src" directory (creates 10px indentation for its children)
    let src_dir = root.join("src");
    let _ = app.dispatch(EditorCommand::ToggleDirectoryExpand(src_dir.clone()));
    assert!(app.expanded_dirs.contains(&src_dir));

    let _elem2 = sidebar::render_file_manager(&app);

    // 3. Expand "src/core" directory (creates 20px indentation for its children)
    let core_dir = src_dir.join("core");
    let _ = app.dispatch(EditorCommand::ToggleDirectoryExpand(core_dir.clone()));
    assert!(app.expanded_dirs.contains(&core_dir));

    let _elem3 = sidebar::render_file_manager(&app);
}

#[test]
fn test_render_file_lifecycle_status_orange_and_green() {
    let temp = create_test_workspace();
    let root = temp.path().to_path_buf();
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::OpenWorkspace(root.clone()));

    let main_py = root.join("src/main.py");

    // 1. Select and open file -> Orange Opened state
    let _ = app.dispatch(EditorCommand::SelectFileManagerItem(main_py.clone()));
    assert_eq!(app.file_states.get(&main_py), Some(&FileItemState::Opened));

    // Render with Opened/Orange state
    let _elem_orange = sidebar::render_file_manager(&app);

    // 2. Save file -> Green Saved state
    let _ = app.dispatch(EditorCommand::SaveActiveFile);
    assert_eq!(app.file_states.get(&main_py), Some(&FileItemState::Saved));

    // Render with Saved/Green state
    let _elem_green = sidebar::render_file_manager(&app);

    // 3. Edit buffer -> Reverts to Orange Opened state
    let _ = app.dispatch(EditorCommand::InsertText("# edit\n".to_string()));
    assert_eq!(app.file_states.get(&main_py), Some(&FileItemState::Opened));

    // Render with reverted Orange state
    let _elem_reverted = sidebar::render_file_manager(&app);
}

#[test]
fn test_render_floating_context_menu_for_folder_and_file() {
    let temp = create_test_workspace();
    let root = temp.path().to_path_buf();
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::OpenWorkspace(root.clone()));

    let src_dir = root.join("src");
    let main_py = root.join("src/main.py");

    // 1. Folder context menu
    let folder_menu = FileContextMenuState {
        target_path: src_dir,
        is_dir: true,
        position_x: 240.0,
        position_y: 80.0,
    };
    let _folder_menu_elem = sidebar::render_file_context_menu(&app, &folder_menu);

    // 2. File context menu
    let file_menu = FileContextMenuState {
        target_path: main_py,
        is_dir: false,
        position_x: 240.0,
        position_y: 120.0,
    };
    let _file_menu_elem = sidebar::render_file_context_menu(&app, &file_menu);
}

#[test]
fn test_render_status_bar_toggle_file_manager_button() {
    let mut app = LideApp::new(20.0, 400.0);

    // Renders status bar with closed file manager
    assert!(!app.file_manager_open);
    let _sb1 = status_bar::render_status_bar(&app);

    // Renders status bar with open file manager
    app.file_manager_open = true;
    let _sb2 = status_bar::render_status_bar(&app);
}

#[test]
fn test_render_app_full_shell_with_file_manager_and_context_menu() {
    let temp = create_test_workspace();
    let root = temp.path().to_path_buf();
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::OpenWorkspace(root.clone()));

    // 1. Full shell with file manager open
    assert!(app.file_manager_open);
    let _shell1 = render_app(&app);

    // 2. Full shell with active floating context menu
    app.file_context_menu = Some(FileContextMenuState {
        target_path: root.join("src/main.py"),
        is_dir: false,
        position_x: 200.0,
        position_y: 150.0,
    });
    let _shell2 = render_app(&app);

    // 3. Full shell with active inline create prompt
    app.file_context_menu = None;
    let _ = app.dispatch(EditorCommand::TriggerCreateFilePrompt(root.join("src")));
    assert!(app.new_item_prompt.is_some());
    let _shell3 = render_app(&app);
}
