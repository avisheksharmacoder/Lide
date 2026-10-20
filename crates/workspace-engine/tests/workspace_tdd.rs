use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use lang_contract::{LanguageId, StubResolution};
use tempfile::TempDir;
use workspace_engine::{
    VirtualenvDetector, WorkspaceIndexer, WorkspaceSearcher,
};

/// Helper: Builds a realistic polyglot Python/Cython/Rust workspace on disk.
fn create_sample_workspace() -> TempDir {
    let temp = TempDir::new().expect("Failed to create temp workspace");
    let root = temp.path();

    // Initialize a `.git` folder so `ignore::WalkBuilder` honors `.gitignore` in temp directories
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join(".gitignore"), "ignored_build/\n*.log\n").unwrap();

    // 1. Valid source files across Python, Cython, Binary .pyd, and Rust
    fs::create_dir_all(root.join("src/core")).unwrap();
    fs::write(
        root.join("src/main.py"),
        "import math\n\ndef launch_pipeline(alpha: float) -> float:\n    return alpha * 2.0\n",
    )
    .unwrap();
    fs::write(
        root.join("src/core/fast_ops.pyx"),
        "cdef double cython_dot_product(double a, double b):\n    return a * b\n",
    )
    .unwrap();
    fs::write(
        root.join("src/core/fast_ops.pyi"),
        "def cython_dot_product(a: float, b: float) -> float: ...\n",
    )
    .unwrap();
    // Simulated compiled binary `.pyd` containing null bytes (`\x00`) and fake match text
    fs::write(
        root.join("src/core/fast_ops.cp312-win_amd64.pyd"),
        b"MZ\x90\x00\x03\x00\x00\x00launch_pipeline_binary_trap\x00",
    )
    .unwrap();
    fs::write(
        root.join("src/core/native_ext.rs"),
        "pub fn rust_kernel() -> usize { 42 }\n",
    )
    .unwrap();

    // 2. Noise directories that MUST be pruned automatically
    fs::create_dir_all(root.join("src/__pycache__")).unwrap();
    fs::write(
        root.join("src/__pycache__/main.cpython-312.pyc"),
        b"\x00\x00launch_pipeline",
    )
    .unwrap();

    fs::create_dir_all(root.join("ignored_build")).unwrap();
    fs::write(
        root.join("ignored_build/generated.py"),
        "def launch_pipeline(): pass\n",
    )
    .unwrap();
    fs::write(root.join("debug.log"), "launch_pipeline log entry\n").unwrap();

    temp
}

// ============================================================================
// 1. FAST DIRECTORY WALKING, GITIGNORE PRUNING & POLYGLOT CLASSIFICATION
// ============================================================================

#[test]
fn test_workspace_indexer_prunes_gitignore_and_pycache_and_classifies_polyglot_files() {
    let temp = create_sample_workspace();
    let indexer = WorkspaceIndexer::new();

    let start = Instant::now();
    let files = indexer.index_directory(temp.path());
    let elapsed = start.elapsed();

    let rel_paths: Vec<String> = files
        .iter()
        .map(|f| f.relative_path.to_string_lossy().replace('\\', "/"))
        .collect();

    // Must include valid Python, Cython, Stub, Binary .pyd, and Rust files
    assert!(rel_paths.contains(&"src/main.py".to_string()));
    assert!(rel_paths.contains(&"src/core/fast_ops.pyx".to_string()));
    assert!(rel_paths.contains(&"src/core/fast_ops.pyi".to_string()));
    assert!(rel_paths.contains(&"src/core/fast_ops.cp312-win_amd64.pyd".to_string()));
    assert!(rel_paths.contains(&"src/core/native_ext.rs".to_string()));

    // Must prune `__pycache__`, `.gitignore` rules (`ignored_build/`, `*.log`), and `.git`
    assert!(
        !rel_paths.iter().any(|p| p.contains("__pycache__")),
        "Indexer leaked __pycache__ files: {:?}",
        rel_paths
    );
    assert!(
        !rel_paths.iter().any(|p| p.contains("ignored_build")),
        "Indexer failed to respect .gitignore directory rule: {:?}",
        rel_paths
    );
    assert!(
        !rel_paths.iter().any(|p| p.ends_with(".log")),
        "Indexer failed to respect .gitignore glob rule: {:?}",
        rel_paths
    );

    // Verify language classification for polyglot roadmap
    let find_lang = |rel: &str| {
        files
            .iter()
            .find(|f| f.relative_path.to_string_lossy().replace('\\', "/") == rel)
            .and_then(|f| f.language)
    };
    assert_eq!(find_lang("src/main.py"), Some(LanguageId::Python));
    assert_eq!(find_lang("src/core/fast_ops.pyi"), Some(LanguageId::Python));
    assert_eq!(find_lang("src/core/fast_ops.pyx"), Some(LanguageId::Cython));
    assert_eq!(
        find_lang("src/core/fast_ops.cp312-win_amd64.pyd"),
        Some(LanguageId::PydBinaryStub)
    );
    assert_eq!(find_lang("src/core/native_ext.rs"), Some(LanguageId::Rust));

    assert!(
        elapsed.as_millis() < 15,
        "Workspace indexing breached 15ms SLA: took {:?}",
        elapsed
    );
}

#[test]
fn test_workspace_indexer_resolves_live_pyd_binary_to_sibling_pyi_or_pyx() {
    let temp = create_sample_workspace();
    let indexer = WorkspaceIndexer::new();
    let pyd_path = temp.path().join("src/core/fast_ops.cp312-win_amd64.pyd");

    // 1. When both `.pyi` and `.pyx` sit next to `.pyd` on disk, `.pyi` wins
    let res = indexer.resolve_binary_artifact(&pyd_path);
    assert_eq!(
        res,
        StubResolution::StubFile(temp.path().join("src/core/fast_ops.pyi"))
    );

    // 2. Remove `.pyi` on disk -> automatically falls back to `.pyx` Cython implementation
    fs::remove_file(temp.path().join("src/core/fast_ops.pyi")).unwrap();
    let res_fallback = indexer.resolve_binary_artifact(&pyd_path);
    assert_eq!(
        res_fallback,
        StubResolution::NativeSource {
            path: temp.path().join("src/core/fast_ops.pyx"),
            language: LanguageId::Cython,
        }
    );
}

// ============================================================================
// 2. RIPGREP (`grep-searcher`) REGEX SEARCH & BINARY SKIPPING TESTS
// ============================================================================

#[test]
fn test_workspace_searcher_finds_regex_matches_and_skips_binary_pyd_files() {
    let temp = create_sample_workspace();
    let searcher = WorkspaceSearcher::new();

    // Search for `launch_pipeline` across the workspace:
    // - Present in `src/main.py` at line 3, col 4 ("def launch_pipeline(alpha: float) -> float:")
    // - Also present inside binary `fast_ops.cp312-win_amd64.pyd` (after null bytes) -> MUST BE SKIPPED
    // - Also present inside `ignored_build/generated.py` and `debug.log` -> MUST BE SKIPPED
    let matches = searcher
        .search_regex(temp.path(), r"launch_pipeline\([a-z]+", 100)
        .expect("Regex search should succeed");

    assert_eq!(
        matches.len(),
        1,
        "Must only match src/main.py and skip binary .pyd and .gitignore files, got: {:?}",
        matches
    );

    let hit = &matches[0];
    assert_eq!(
        hit.relative_path.to_string_lossy().replace('\\', "/"),
        "src/main.py"
    );
    assert_eq!(hit.line_number, 3);
    assert_eq!(hit.column, 4);
    assert_eq!(hit.line_text, "def launch_pipeline(alpha: float) -> float:");
}

#[test]
fn test_workspace_searcher_enforces_max_results_cap_to_protect_memory() {
    let temp = create_sample_workspace();
    let searcher = WorkspaceSearcher::new();

    // Cap at 2 matches even when searching for a common token (`\b(def|cdef|pub)\b`)
    let matches = searcher
        .search_regex(temp.path(), r"\b(def|cdef|pub)\b", 2)
        .expect("Regex search should succeed");

    assert_eq!(matches.len(), 2, "Searcher must stop immediately at max_results");
}

// ============================================================================
// 3. ZERO-SUBPROCESS PYTHON VIRTUALENV DISCOVERY (`pyvenv.cfg`)
// ============================================================================

#[test]
fn test_virtualenv_detector_discovers_local_dot_venv_and_parses_pyvenv_cfg() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    // Simulate a standard `uv` / `venv` layout inside `.venv/`
    let venv_root = root.join(".venv");
    let bin_dir = if cfg!(windows) {
        venv_root.join("Scripts")
    } else {
        venv_root.join("bin")
    };
    let interpreter = if cfg!(windows) {
        bin_dir.join("python.exe")
    } else {
        bin_dir.join("python")
    };
    let site_packages = if cfg!(windows) {
        venv_root.join("Lib/site-packages")
    } else {
        venv_root.join("lib/python3.12/site-packages")
    };

    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(&site_packages).unwrap();
    fs::write(&interpreter, b"").unwrap();
    fs::write(
        venv_root.join("pyvenv.cfg"),
        "home = /usr/local/bin\ninclude-system-site-packages = false\nversion_info = 3.12.6\n",
    )
    .unwrap();

    let detector = VirtualenvDetector::new();
    let info = detector
        .detect(root, None)
        .expect("Should detect .venv in workspace root");

    assert_eq!(info.root, venv_root);
    assert_eq!(info.interpreter_path, interpreter);
    assert_eq!(info.site_packages, Some(site_packages));
    assert_eq!(info.version.as_deref(), Some("3.12.6"));
}

#[test]
fn test_virtualenv_detector_prioritizes_active_env_override_over_workspace_venv() {
    let temp = TempDir::new().unwrap();
    let workspace_root = temp.path().join("project");
    let external_conda_env = temp.path().join("external_env");

    // Create both `.venv` in workspace AND an external override env
    for (env_dir, ver) in [(&workspace_root.join(".venv"), "3.11.0"), (&external_conda_env, "3.13.1")] {
        let bin_dir = if cfg!(windows) {
            env_dir.join("Scripts")
        } else {
            env_dir.join("bin")
        };
        let py_bin = if cfg!(windows) {
            bin_dir.join("python.exe")
        } else {
            bin_dir.join("python")
        };
        fs::create_dir_all(&bin_dir).unwrap();
        fs::write(&py_bin, b"").unwrap();
        fs::write(env_dir.join("pyvenv.cfg"), format!("version = {ver}\n")).unwrap();
    }

    let detector = VirtualenvDetector::new();
    let info = detector
        .detect(&workspace_root, Some(&external_conda_env))
        .expect("Should resolve active_env_override");

    assert_eq!(info.root, PathBuf::from(&external_conda_env));
    assert_eq!(info.version.as_deref(), Some("3.13.1"));
}
