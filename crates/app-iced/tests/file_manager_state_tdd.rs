use std::fs;
use std::path::PathBuf;

use app_iced::{EditorCommand, FileItemState, LideApp};
use tempfile::TempDir;

/// Helper: Creates a mock directory tree for testing TEA file manager state transitions.
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
fn test_toggle_file_manager_visibility() {
    let mut app = LideApp::new(20.0, 400.0);
    assert!(!app.file_manager_open);

    let _ = app.dispatch(EditorCommand::ToggleFileManager);
    assert!(app.file_manager_open);

    let _ = app.dispatch(EditorCommand::ToggleFileManager);
    assert!(!app.file_manager_open);
}

#[test]
fn test_open_workspace_populates_tree_and_enables_file_manager() {
    let temp = create_test_workspace();
    let root = temp.path().to_path_buf();
    let mut app = LideApp::new(20.0, 400.0);

    let _ = app.dispatch(EditorCommand::OpenWorkspace(root.clone()));

    assert!(app.file_manager_open, "Opening a workspace should automatically open file manager");
    assert!(app.workspace_tree.is_some(), "workspace_tree must be populated");

    let tree = app.workspace_tree.as_ref().unwrap();
    assert_eq!(tree.path, root);
    let child_names: Vec<&str> = tree.children.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(child_names, vec!["src", "README.md"]);
}

#[test]
fn test_directory_expand_and_collapse_in_state() {
    let temp = create_test_workspace();
    let root = temp.path().to_path_buf();
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::OpenWorkspace(root.clone()));

    let src_path = root.join("src");
    assert!(!app.expanded_dirs.contains(&src_path));

    // Toggle expand
    let _ = app.dispatch(EditorCommand::ToggleDirectoryExpand(src_path.clone()));
    assert!(app.expanded_dirs.contains(&src_path));

    // Toggle collapse
    let _ = app.dispatch(EditorCommand::ToggleDirectoryExpand(src_path.clone()));
    assert!(!app.expanded_dirs.contains(&src_path));
}

#[test]
fn test_file_lifecycle_status_transitions_orange_and_green() {
    let temp = create_test_workspace();
    let root = temp.path().to_path_buf();
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::OpenWorkspace(root.clone()));

    let main_py = root.join("src/main.py");

    // 1. Initial state: File is not yet opened, state is None or Normal
    assert_eq!(app.file_states.get(&main_py).copied().unwrap_or_default(), FileItemState::Normal);

    // 2. User selects and opens file: Must become FileItemState::Opened (ORANGE)
    let _ = app.dispatch(EditorCommand::SelectFileManagerItem(main_py.clone()));
    assert_eq!(app.active_file_path, Some(main_py.clone()));
    assert_eq!(
        app.file_states.get(&main_py),
        Some(&FileItemState::Opened),
        "Selected and opened file must be marked Opened (Orange)"
    );

    // 3. User saves file: Must transition to FileItemState::Saved (GREEN)
    let _ = app.dispatch(EditorCommand::SaveActiveFile);
    assert_eq!(
        app.file_states.get(&main_py),
        Some(&FileItemState::Saved),
        "Saved file must be marked Saved (Green)"
    );

    // 4. User modifies buffer: Must revert to FileItemState::Opened (ORANGE) to indicate unsaved changes
    let _ = app.dispatch(EditorCommand::InsertText("# New edit\n".to_string()));
    assert!(app.is_dirty);
    assert_eq!(
        app.file_states.get(&main_py),
        Some(&FileItemState::Opened),
        "Edited file must transition back to Opened (Orange)"
    );

    // 5. User saves again: Returns to FileItemState::Saved (GREEN)
    let _ = app.dispatch(EditorCommand::SaveActiveFile);
    assert!(!app.is_dirty);
    assert_eq!(
        app.file_states.get(&main_py),
        Some(&FileItemState::Saved),
        "Re-saved file must transition back to Saved (Green)"
    );
}

#[test]
fn test_file_context_menu_open_and_close() {
    let mut app = LideApp::new(20.0, 400.0);
    assert!(app.file_context_menu.is_none());

    let test_file = PathBuf::from("/workspace/src/app.py");

    // 1. Open context menu
    let _ = app.dispatch(EditorCommand::OpenFileContextMenu {
        path: test_file.clone(),
        is_dir: false,
        position_x: 120.0,
        position_y: 250.0,
    });

    let menu = app.file_context_menu.as_ref().expect("Context menu must be open");
    assert_eq!(menu.target_path, test_file);
    assert!(!menu.is_dir);
    assert_eq!(menu.position_x, 120.0);
    assert_eq!(menu.position_y, 250.0);

    // 2. Close context menu
    let _ = app.dispatch(EditorCommand::CloseFileContextMenu);
    assert!(app.file_context_menu.is_none());

    // 3. Escape key closes context menu as well
    let _ = app.dispatch(EditorCommand::OpenFileContextMenu {
        path: test_file,
        is_dir: false,
        position_x: 50.0,
        position_y: 60.0,
    });
    assert!(app.file_context_menu.is_some());
    let _ = app.dispatch(EditorCommand::Escape);
    assert!(app.file_context_menu.is_none());
}

#[test]
fn test_inline_create_file_under_folder() {
    let temp = create_test_workspace();
    let root = temp.path().to_path_buf();
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::OpenWorkspace(root.clone()));

    let src_dir = root.join("src");

    // 1. Trigger create file prompt
    let _ = app.dispatch(EditorCommand::TriggerCreateFilePrompt(src_dir.clone()));
    let prompt = app.new_item_prompt.as_ref().expect("Prompt must be active");
    assert_eq!(prompt.parent_dir, src_dir);
    assert!(!prompt.is_folder);
    assert!(app.expanded_dirs.contains(&src_dir), "Parent folder must auto-expand");

    // 2. Update name
    let _ = app.dispatch(EditorCommand::UpdateNewItemName("service.py".to_string()));
    assert_eq!(app.new_item_prompt.as_ref().unwrap().input_value, "service.py");

    // 3. Confirm creation
    let _ = app.dispatch(EditorCommand::ConfirmCreateItem);
    assert!(app.new_item_prompt.is_none());

    let created_file = src_dir.join("service.py");
    assert!(created_file.is_file(), "File must exist on disk");

    // Automatically opened in editor
    assert_eq!(app.active_file_path, Some(created_file.clone()));
    assert_eq!(app.file_states.get(&created_file), Some(&FileItemState::Opened));

    // Tree was updated
    let tree = app.workspace_tree.as_ref().unwrap();
    let src_node = tree.children.iter().find(|c| c.name == "src").unwrap();
    assert!(src_node.children.iter().any(|c| c.name == "service.py"));
}

#[test]
fn test_inline_create_folder_under_workspace() {
    let temp = create_test_workspace();
    let root = temp.path().to_path_buf();
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::OpenWorkspace(root.clone()));

    // 1. Trigger create folder prompt at root
    let _ = app.dispatch(EditorCommand::TriggerCreateFolderPrompt(root.clone()));
    assert!(app.new_item_prompt.as_ref().unwrap().is_folder);

    // 2. Enter folder name and confirm
    let _ = app.dispatch(EditorCommand::UpdateNewItemName("tests".to_string()));
    let _ = app.dispatch(EditorCommand::ConfirmCreateItem);

    let created_folder = root.join("tests");
    assert!(created_folder.is_dir(), "Folder must exist on disk");

    // Tree was updated with "tests" folder (sorted with directories first)
    let tree = app.workspace_tree.as_ref().unwrap();
    assert!(tree.children.iter().any(|c| c.name == "tests" && c.is_dir));
}

#[test]
fn test_delete_workspace_entry_and_editor_cleanup() {
    let temp = create_test_workspace();
    let root = temp.path().to_path_buf();
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::OpenWorkspace(root.clone()));

    let main_py = root.join("src/main.py");
    let _ = app.dispatch(EditorCommand::SelectFileManagerItem(main_py.clone()));
    assert_eq!(app.active_file_path, Some(main_py.clone()));

    // Delete active file
    let _ = app.dispatch(EditorCommand::DeleteWorkspaceEntry(main_py.clone()));
    assert!(!main_py.exists(), "File must be removed from disk");

    // Active file should be cleared from editor
    assert_eq!(app.active_file_path, None);
    assert_eq!(app.file_states.get(&main_py), None);

    // Tree should no longer have main.py
    let tree = app.workspace_tree.as_ref().unwrap();
    let src_node = tree.children.iter().find(|c| c.name == "src").unwrap();
    assert!(!src_node.children.iter().any(|c| c.name == "main.py"));
}
