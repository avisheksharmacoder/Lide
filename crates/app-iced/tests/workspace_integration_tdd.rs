use std::fs;
use app_iced::{EditorCommand, LideApp, SidebarTab};
use tempfile::TempDir;

/// Helper: Creates a temporary Python workspace with a `.venv` (`pyvenv.cfg`),
/// a Python source module, a Cython `.pyx` module, a `.pyi` stub, and a binary `.pyd` extension.
fn build_mock_python_workspace() -> TempDir {
    let temp = TempDir::new().expect("Failed to create temp workspace");
    let root = temp.path();

    // 1. Setup `.git` and `.gitignore`
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join(".gitignore"), "__pycache__/\nbuild/\n").unwrap();

    // 2. Setup `.venv` with `pyvenv.cfg` and mock interpreter binary
    let venv_root = root.join(".venv");
    let bin_dir = if cfg!(windows) {
        venv_root.join("Scripts")
    } else {
        venv_root.join("bin")
    };
    let py_bin = if cfg!(windows) {
        bin_dir.join("python.exe")
    } else {
        bin_dir.join("python")
    };
    fs::create_dir_all(&bin_dir).unwrap();
    fs::write(&py_bin, b"").unwrap();
    fs::write(
        venv_root.join("pyvenv.cfg"),
        "home = /usr/bin\ninclude-system-site-packages = false\nversion_info = 3.12.6\n",
    )
    .unwrap();

    // 3. Setup workspace source files and a compiled `.pyd` binary with adjacent `.pyi` stub
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/engine.py"),
        concat!(
            "class TensorRunner:\n",
            "    def run_step(self, lr: float) -> float:\n",
            "        return lr * 0.95\n"
        ),
    )
    .unwrap();

    fs::write(
        root.join("src/fast_kernel.pyi"),
        "def accelerate_matrix(dim: int) -> float: ...\n",
    )
    .unwrap();
    fs::write(
        root.join("src/fast_kernel.cp312-win_amd64.pyd"),
        b"MZ\x90\x00\x03\x00\x00\x00BINARY_DLL_PAYLOAD",
    )
    .unwrap();

    temp
}

#[test]
fn test_open_workspace_indexes_files_and_displays_virtualenv_version_in_status_bar() {
    let temp = build_mock_python_workspace();
    let mut app = LideApp::new(20.0, 400.0);

    let _ = app.dispatch(EditorCommand::OpenWorkspace(temp.path().to_path_buf()));

    // 1. Verify discovered Python virtualenv
    let env = app
        .python_env
        .as_ref()
        .expect("Must detect .venv in workspace");
    assert_eq!(env.version.as_deref(), Some("3.12.6"));

    // 2. Verify status bar displays detected Python version and env directory name
    let status = &app.status_bar_text;
    assert!(
        status.contains("Python 3.12.6 (.venv)"),
        "Status bar must show detected virtualenv version and folder name, got: {status}"
    );

    // 3. Verify indexed workspace files
    let rel_paths: Vec<String> = app
        .workspace_files
        .iter()
        .map(|f| f.rel_path.to_string_lossy().replace('\\', "/"))
        .collect();
    assert!(rel_paths.contains(&"src/engine.py".to_string()));
    assert!(rel_paths.contains(&"src/fast_kernel.pyi".to_string()));
    assert!(rel_paths.contains(&"src/fast_kernel.cp312-win_amd64.pyd".to_string()));
}

#[test]
fn test_switching_sidebar_tabs_between_files_and_structure() {
    let temp = build_mock_python_workspace();
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::OpenWorkspace(temp.path().to_path_buf()));

    assert_eq!(app.active_sidebar_tab, SidebarTab::ProjectFiles);

    let _ = app.dispatch(EditorCommand::SwitchSidebarTab(SidebarTab::Structure));
    assert_eq!(app.active_sidebar_tab, SidebarTab::Structure);

    let _ = app.dispatch(EditorCommand::SwitchSidebarTab(SidebarTab::ProjectFiles));
    assert_eq!(app.active_sidebar_tab, SidebarTab::ProjectFiles);
}

#[test]
fn test_opening_python_file_from_sidebar_loads_buffer_and_populates_structure_outline() {
    let temp = build_mock_python_workspace();
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::OpenWorkspace(temp.path().to_path_buf()));

    let engine_py = temp.path().join("src/engine.py");
    let _ = app.dispatch(EditorCommand::OpenWorkspaceFile(engine_py.clone()));

    assert_eq!(app.active_file_path, Some(engine_py));

    // Buffer viewport must now display `src/engine.py`
    assert_eq!(app.visible_lines[0].text, "class TensorRunner:");

    // Structure outline must automatically contain `TensorRunner` and `run_step`
    assert_eq!(app.outline_items.len(), 2);
    assert_eq!(app.outline_items[0].name, "TensorRunner");
    assert_eq!(app.outline_items[1].name, "run_step");
}

#[test]
fn test_clicking_binary_pyd_file_in_sidebar_automatically_opens_resolved_pyi_stub() {
    let temp = build_mock_python_workspace();
    let mut app = LideApp::new(20.0, 400.0);
    let _ = app.dispatch(EditorCommand::OpenWorkspace(temp.path().to_path_buf()));

    let binary_pyd = temp.path().join("src/fast_kernel.cp312-win_amd64.pyd");
    let expected_pyi = temp.path().join("src/fast_kernel.pyi");

    // User clicks `.pyd` binary in the Project Files sidebar
    let _ = app.dispatch(EditorCommand::OpenWorkspaceFile(binary_pyd));

    // Interceptor must redirect to `fast_kernel.pyi` and load its typing stub text
    assert_eq!(app.active_file_path, Some(expected_pyi));
    assert_eq!(
        app.visible_lines[0].text,
        "def accelerate_matrix(dim: int) -> float: ..."
    );
}
