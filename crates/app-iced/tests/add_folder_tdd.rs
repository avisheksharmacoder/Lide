//! TDD test suite verifying the "Add Folder / Open Folder" feature in Lide File Manager.
//!
//! Covers:
//! 1. OpenFolderPrompt state lifecycle (Trigger, Update, Cancel, Escape).
//! 2. Opening an existing folder from the prompt.
//! 3. Creating a brand new folder on disk if it does not exist, then opening it.
//! 4. Empty/whitespace path validation and error reporting.
//! 5. Headless UI rendering of both empty state and active open folder prompt.

use std::fs;
use std::path::PathBuf;

use app_iced::views::sidebar;
use app_iced::{EditorCommand, FileItemState, LideApp, OpenFolderPromptState};
use tempfile::TempDir;

#[test]
fn test_open_folder_prompt_lifecycle_and_escape() {
    let mut app = LideApp::new(20.0, 400.0);
    assert!(app.open_folder_prompt.is_none());

    // 1. Trigger open folder prompt
    let _ = app.dispatch(EditorCommand::TriggerOpenFolderPrompt);
    assert!(app.open_folder_prompt.is_some());
    let prompt = app.open_folder_prompt.as_ref().unwrap();
    assert!(!prompt.input_value.is_empty(), "Default path should be populated");
    assert_eq!(prompt.error_message, None);

    // 2. Update path
    let _ = app.dispatch(EditorCommand::UpdateOpenFolderPath("/custom/test/path".to_string()));
    assert_eq!(
        app.open_folder_prompt.as_ref().unwrap().input_value,
        "/custom/test/path"
    );

    // 3. Cancel prompt
    let _ = app.dispatch(EditorCommand::CancelOpenFolderPrompt);
    assert!(app.open_folder_prompt.is_none());

    // 4. Trigger again, then press Escape to dismiss
    let _ = app.dispatch(EditorCommand::TriggerOpenFolderPrompt);
    assert!(app.open_folder_prompt.is_some());
    let _ = app.dispatch(EditorCommand::Escape);
    assert!(app.open_folder_prompt.is_none());
}

#[test]
fn test_open_existing_directory_populates_workspace() {
    let temp = TempDir::new().expect("Failed to create temp directory");
    let root = temp.path().to_path_buf();

    fs::write(root.join("main.py"), "print('hello')\n").unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.py"), "def foo(): pass\n").unwrap();

    let mut app = LideApp::new(20.0, 400.0);
    assert!(app.active_workspace_root.is_none());

    // Trigger prompt and enter path
    let _ = app.dispatch(EditorCommand::TriggerOpenFolderPrompt);
    let _ = app.dispatch(EditorCommand::UpdateOpenFolderPath(root.to_string_lossy().to_string()));

    // Confirm
    let _ = app.dispatch(EditorCommand::ConfirmOpenFolder);

    // Prompt closed, workspace opened
    assert!(app.open_folder_prompt.is_none());
    assert!(app.active_workspace_root.is_some());
    assert!(app.file_manager_open);
    assert!(app.workspace_tree.is_some());

    let tree = app.workspace_tree.as_ref().unwrap();
    assert!(tree.children.iter().any(|c| c.name == "src" && c.is_dir));
    assert!(tree.children.iter().any(|c| c.name == "main.py" && !c.is_dir));
}

#[test]
fn test_add_new_folder_creates_directory_on_disk_and_opens_workspace() {
    let temp = TempDir::new().expect("Failed to create temp directory");
    let target_new_folder = temp.path().join("my_new_project");
    assert!(!target_new_folder.exists(), "Target folder must not exist before test");

    let mut app = LideApp::new(20.0, 400.0);

    // 1. Enter path for non-existent folder
    let _ = app.dispatch(EditorCommand::TriggerOpenFolderPrompt);
    let _ = app.dispatch(EditorCommand::UpdateOpenFolderPath(
        target_new_folder.to_string_lossy().to_string(),
    ));

    // 2. Confirm: must create folder on disk and open it as workspace
    let _ = app.dispatch(EditorCommand::ConfirmOpenFolder);

    assert!(target_new_folder.is_dir(), "Folder must be created on disk");
    assert!(app.open_folder_prompt.is_none());
    assert!(app.active_workspace_root.is_some());
    assert!(app.file_manager_open);
    assert!(app.workspace_tree.is_some());

    // 3. User can now start working immediately: create a file inside the new project!
    let _ = app.dispatch(EditorCommand::TriggerCreateFilePrompt(target_new_folder.clone()));
    let _ = app.dispatch(EditorCommand::UpdateNewItemName("app.py".to_string()));
    let _ = app.dispatch(EditorCommand::ConfirmCreateItem);

    let created_file = target_new_folder.join("app.py");
    assert!(created_file.is_file(), "File app.py must exist on disk");
    assert_eq!(app.active_file_path, Some(created_file.clone()));
    assert_eq!(app.file_states.get(&created_file), Some(&FileItemState::Opened));
}

#[test]
fn test_open_folder_empty_path_validation() {
    let mut app = LideApp::new(20.0, 400.0);

    // 1. Trigger prompt
    let _ = app.dispatch(EditorCommand::TriggerOpenFolderPrompt);

    // 2. Set whitespace-only path
    let _ = app.dispatch(EditorCommand::UpdateOpenFolderPath("   \t  ".to_string()));
    let _ = app.dispatch(EditorCommand::ConfirmOpenFolder);

    // Must not open workspace, error message must be set
    assert!(app.active_workspace_root.is_none());
    assert!(app.open_folder_prompt.is_some());
    let prompt = app.open_folder_prompt.as_ref().unwrap();
    assert!(prompt.error_message.is_some());
}

#[test]
fn test_render_sidebar_empty_state_and_open_folder_prompt() {
    let mut app = LideApp::new(20.0, 400.0);
    app.file_manager_open = true;

    // 1. Render when no workspace is open and prompt is inactive (shows "Open Folder..." button)
    let _elem_empty = sidebar::render_file_manager(&app);

    // 2. Render when open folder prompt is active
    app.open_folder_prompt = Some(OpenFolderPromptState {
        input_value: "C:\\projects\\demo".to_string(),
        error_message: None,
    });
    let _elem_prompt = sidebar::render_file_manager(&app);

    // 3. Render when open folder prompt has an error
    app.open_folder_prompt = Some(OpenFolderPromptState {
        input_value: "".to_string(),
        error_message: Some("Folder path cannot be empty".to_string()),
    });
    let _elem_err = sidebar::render_file_manager(&app);
}

#[test]
fn test_keyboard_shortcut_ctrl_o_triggers_open_folder_prompt() {
    use app_iced::subscriptions::keyboard::resolve_key_action;
    use iced::keyboard::{Key, Modifiers};

    let cmd = resolve_key_action(Key::Character("o".into()), Modifiers::CTRL)
        .expect("Ctrl+O must resolve to command");
    assert_eq!(cmd, EditorCommand::TriggerOpenFolderPrompt);

    let mut app = LideApp::new(20.0, 400.0);
    assert!(app.open_folder_prompt.is_none());

    let _ = app.dispatch(cmd);
    assert!(app.open_folder_prompt.is_some());
}
