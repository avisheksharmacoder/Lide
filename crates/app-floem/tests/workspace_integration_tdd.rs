use std::cell::Cell;
use std::fs;
use std::rc::Rc;

use app_floem::{EditorCommand, EditorViewModel, SidebarTab};
use floem::reactive::{create_effect, SignalGet};
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

// ============================================================================
// 1. WORKSPACE INDEXING & VIRTUALENV VERSION IN STATUS BAR
// ============================================================================

#[test]
fn test_open_workspace_indexes_files_and_displays_virtualenv_version_in_status_bar() {
    let temp = build_mock_python_workspace();
    let mut vm = EditorViewModel::new(20.0, 400.0);

    vm.dispatch(EditorCommand::OpenWorkspace(temp.path().to_path_buf()));

    // 1. Verify discovered Python virtualenv signal
    let env = vm
        .signals
        .python_env
        .get()
        .expect("Must detect .venv in workspace");
    assert_eq!(env.version.as_deref(), Some("3.12.6"));

    // 2. Verify status bar displays the detected Python version and env directory name
    let status = vm.signals.status_bar_text.get();
    assert!(
        status.contains("Python 3.12.6 (.venv)"),
        "Status bar must show detected virtualenv version and folder name, got: {status}"
    );

    // 3. Verify indexed workspace files in the sidebar signal
    let files = vm.signals.workspace_files.get();
    let rel_paths: Vec<String> = files
        .iter()
        .map(|f| f.relative_path.to_string_lossy().replace('\\', "/"))
        .collect();
    assert!(rel_paths.contains(&"src/engine.py".to_string()));
    assert!(rel_paths.contains(&"src/fast_kernel.pyi".to_string()));
    assert!(rel_paths.contains(&"src/fast_kernel.cp312-win_amd64.pyd".to_string()));
}

// ============================================================================
// 2. SIDEBAR TAB SWITCHING & REACTIVE SIGNAL ISOLATION
// ============================================================================

#[test]
fn test_switching_sidebar_tabs_between_files_and_structure_does_not_redraw_viewport() {
    let temp = build_mock_python_workspace();
    let mut vm = EditorViewModel::new(20.0, 400.0);
    vm.dispatch(EditorCommand::OpenWorkspace(temp.path().to_path_buf()));

    let tab_notifications = Rc::new(Cell::new(0usize));
    let viewport_notifications = Rc::new(Cell::new(0usize));

    let tab_sig = vm.signals.active_sidebar_tab;
    let tab_count = tab_notifications.clone();
    create_effect(move |_| {
        let _ = tab_sig.get();
        tab_count.set(tab_count.get() + 1);
    });

    let vp_sig = vm.signals.visible_lines;
    let vp_count = viewport_notifications.clone();
    create_effect(move |_| {
        let _ = vp_sig.get();
        vp_count.set(vp_count.get() + 1);
    });

    assert_eq!(tab_notifications.get(), 1);
    assert_eq!(viewport_notifications.get(), 1);

    // Switch to ProjectFiles tab, then to Structure tab, then back to ProjectFiles
    vm.dispatch(EditorCommand::SwitchSidebarTab(SidebarTab::Structure));
    assert_eq!(vm.signals.active_sidebar_tab.get(), SidebarTab::Structure);

    vm.dispatch(EditorCommand::SwitchSidebarTab(SidebarTab::ProjectFiles));
    assert_eq!(vm.signals.active_sidebar_tab.get(), SidebarTab::ProjectFiles);

    // Viewport signal must NOT have re-fired during sidebar tab toggles!
    assert_eq!(
        viewport_notifications.get(),
        1,
        "Switching sidebar tabs must never trigger a code viewport redraw"
    );
}

// ============================================================================
// 3. OPENING FILES FROM SIDEBAR & AUTOMATIC .PYD STUB INTERCEPTION
// ============================================================================

#[test]
fn test_opening_python_file_from_sidebar_loads_buffer_and_populates_structure_outline() {
    let temp = build_mock_python_workspace();
    let mut vm = EditorViewModel::new(20.0, 400.0);
    vm.dispatch(EditorCommand::OpenWorkspace(temp.path().to_path_buf()));

    let engine_py = temp.path().join("src/engine.py");
    vm.dispatch(EditorCommand::OpenWorkspaceFile(engine_py.clone()));

    assert_eq!(vm.signals.active_file_path.get(), Some(engine_py));

    // Buffer viewport must now display `src/engine.py`
    let visible = vm.signals.visible_lines.get();
    assert_eq!(visible[0].text, "class TensorRunner:");

    // Structure outline must automatically contain `TensorRunner` and `run_step`
    let outline = vm.signals.outline_items.get();
    assert_eq!(outline.len(), 2);
    assert_eq!(outline[0].name, "TensorRunner");
    assert_eq!(outline[1].name, "run_step");
}

#[test]
fn test_clicking_binary_pyd_file_in_sidebar_automatically_opens_resolved_pyi_stub() {
    let temp = build_mock_python_workspace();
    let mut vm = EditorViewModel::new(20.0, 400.0);
    vm.dispatch(EditorCommand::OpenWorkspace(temp.path().to_path_buf()));

    let binary_pyd = temp.path().join("src/fast_kernel.cp312-win_amd64.pyd");
    let expected_pyi = temp.path().join("src/fast_kernel.pyi");

    // User clicks the `.pyd` binary in the Project Files sidebar
    vm.dispatch(EditorCommand::OpenWorkspaceFile(binary_pyd));

    // Interceptor must redirect to `fast_kernel.pyi` and load its typing stub text
    assert_eq!(vm.signals.active_file_path.get(), Some(expected_pyi));
    let visible = vm.signals.visible_lines.get();
    assert_eq!(
        visible[0].text,
        "def accelerate_matrix(dim: int) -> float: ..."
    );
}
