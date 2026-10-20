use std::fs;
use app_floem::{EditorCommand, EditorViewModel};
use editor_core::Position;
use floem::reactive::SignalGet;
use tempfile::TempDir;

#[test]
fn test_dirty_state_lifecycle_and_atomic_ruff_format_on_save_to_disk() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("pipeline.py");
    fs::write(&file_path, "def run(x: int) -> int:\n    return x\n").unwrap();

    let mut vm = EditorViewModel::new(20.0, 400.0);
    vm.dispatch(EditorCommand::OpenWorkspace(temp.path().to_path_buf()));
    vm.dispatch(EditorCommand::OpenWorkspaceFile(file_path.clone()));

    // 1. Freshly opened file is NOT dirty
    assert!(!vm.signals.is_dirty.get());
    assert!(!vm.signals.status_bar_text.get().contains("[Modified]"));

    // 2. User types a messy unformatted line using auto-indent
    vm.dispatch(EditorCommand::SetCursor(Position::new(0, 23))); // end of `def run(x: int) -> int:`
    vm.dispatch(EditorCommand::InsertNewlineAutoIndent);
    vm.dispatch(EditorCommand::InsertText("y=x*2+1".to_string()));

    assert!(
        vm.signals.is_dirty.get(),
        "Buffer must be marked dirty after editing"
    );
    assert!(
        vm.signals.status_bar_text.get().contains("[Modified]"),
        "Status bar must show [Modified] indicator when buffer is dirty"
    );

    // 3. User presses Ctrl+S (`SaveActiveFile`):
    //    Must format `y=x*2+1` -> `y = x * 2 + 1` via Ruff, write atomically to disk, and clear `is_dirty`!
    vm.dispatch(EditorCommand::SaveActiveFile);

    assert!(
        !vm.signals.is_dirty.get(),
        "Saving active file must reset is_dirty to false"
    );
    assert!(!vm.signals.status_bar_text.get().contains("[Modified]"));

    let saved_on_disk = fs::read_to_string(&file_path).expect("File should exist on disk");
    assert_eq!(
        saved_on_disk,
        "def run(x: int) -> int:\n    y = x * 2 + 1\n    return x\n"
    );
}

#[test]
fn test_save_active_file_writes_raw_buffer_even_when_syntax_is_incomplete() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("wip.py");
    fs::write(&file_path, "x = 1\n").unwrap();

    let mut vm = EditorViewModel::new(20.0, 400.0);
    vm.dispatch(EditorCommand::OpenWorkspaceFile(file_path.clone()));

    // User types an incomplete function signature and hits Ctrl+S before finishing the line
    vm.dispatch(EditorCommand::SetCursor(Position::new(0, 5)));
    vm.dispatch(EditorCommand::InsertText("\ndef unfinished(a, b".to_string()));
    assert!(vm.signals.is_dirty.get());

    vm.dispatch(EditorCommand::SaveActiveFile);

    // Ruff formatter must report `last_format_error`, but disk save MUST still succeed so work is never lost!
    assert!(vm.signals.last_format_error.get().is_some());
    assert!(!vm.signals.is_dirty.get());

    let disk_text = fs::read_to_string(&file_path).unwrap();
    assert_eq!(disk_text, "x = 1\ndef unfinished(a, b\n");
}
