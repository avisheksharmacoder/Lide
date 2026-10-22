use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;
use tokio::sync::{mpsc, watch};

use grep_matcher::Matcher;
use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use ignore::{DirEntry, WalkBuilder};
use lang_contract::{BinaryStubInterceptor, LanguageId, StubResolution};
use lang_python::PydStubResolver;

/// A discovered file in the project workspace with its polyglot language classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceFile {
    /// Absolute path on disk.
    pub path: PathBuf,
    /// Path relative to the workspace root for compact sidebar display.
    pub relative_path: PathBuf,
    /// Alias for backwards compatibility with tests expecting `rel_path`.
    pub rel_path: PathBuf,
    /// Detected language (Python, Cython, PydBinaryStub, C, Cpp, Rust).
    pub language: Option<LanguageId>,
}

/// A node in the hierarchical workspace tree (folder or file).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceTreeNode {
    /// Absolute path on disk.
    pub path: PathBuf,
    /// Path relative to the workspace root.
    pub relative_path: PathBuf,
    /// File or directory name for UI display.
    pub name: String,
    /// True if directory, false if file.
    pub is_dir: bool,
    /// Detected language (None for directories or unclassified files).
    pub language: Option<LanguageId>,
    /// Nested child items (deterministically sorted: directories first, then files).
    pub children: Vec<WorkspaceTreeNode>,
}

/// A flattened visible row from the workspace tree, ready for Iced column rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatTreeItem {
    /// Absolute path on disk.
    pub path: PathBuf,
    /// Path relative to the workspace root.
    pub relative_path: PathBuf,
    /// File or directory name for display.
    pub name: String,
    /// True if directory, false if file.
    pub is_dir: bool,
    /// Nesting depth (0 for top-level children, 1 for children of expanded folders, etc.).
    /// Used directly to calculate the 10px indentation padding: `depth * 10`.
    pub depth: usize,
    /// True if this directory is currently expanded.
    pub is_expanded: bool,
    /// Detected language.
    pub language: Option<LanguageId>,
}

impl WorkspaceTreeNode {
    pub fn new_dir(path: PathBuf, relative_path: PathBuf, name: String) -> Self {
        Self {
            path,
            relative_path,
            name,
            is_dir: true,
            language: None,
            children: Vec::new(),
        }
    }

    pub fn new_file(
        path: PathBuf,
        relative_path: PathBuf,
        name: String,
        language: Option<LanguageId>,
    ) -> Self {
        Self {
            path,
            relative_path,
            name,
            is_dir: false,
            language,
            children: Vec::new(),
        }
    }

    /// Recursively sorts all children: directories first (alphabetical), then files (alphabetical).
    pub fn sort_recursive(&mut self) {
        self.children.sort_by(|a, b| {
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });
        for child in &mut self.children {
            if child.is_dir {
                child.sort_recursive();
            }
        }
    }

    /// Flattens this tree into a linear list of visible items based on currently expanded directory paths.
    /// Indentation depth corresponds to levels of nesting (0, 1, 2, ...).
    pub fn flatten_visible(
        &self,
        expanded_dirs: &std::collections::HashSet<PathBuf>,
    ) -> Vec<FlatTreeItem> {
        let mut items = Vec::new();
        self.flatten_visible_internal(expanded_dirs, 0, &mut items);
        items
    }

    fn flatten_visible_internal(
        &self,
        expanded_dirs: &std::collections::HashSet<PathBuf>,
        depth: usize,
        out: &mut Vec<FlatTreeItem>,
    ) {
        for child in &self.children {
            let is_expanded = child.is_dir && expanded_dirs.contains(&child.path);
            out.push(FlatTreeItem {
                path: child.path.clone(),
                relative_path: child.relative_path.clone(),
                name: child.name.clone(),
                is_dir: child.is_dir,
                depth,
                is_expanded,
                language: child.language,
            });

            if is_expanded {
                child.flatten_visible_internal(expanded_dirs, depth + 1, out);
            }
        }
    }
}

/// A single regex search hit produced by `grep-searcher`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub path: PathBuf,
    pub relative_path: PathBuf,
    /// 1-based line number matching editor gutter coordinates.
    pub line_number: usize,
    /// 0-based byte column where the regex match begins on the line.
    pub column: usize,
    /// Matched line text stripped of trailing CR/LF.
    pub line_text: String,
}

/// Metadata for a discovered Python virtual environment (`.venv`, `venv`, `uv`, Poetry, or `$VIRTUAL_ENV`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PythonEnvInfo {
    /// Root directory of the virtual environment (e.g. `/project/.venv`).
    pub root: PathBuf,
    /// Resolved path to the Python interpreter (`bin/python` on Unix, `Scripts/python.exe` on Windows).
    pub interpreter_path: PathBuf,
    /// Resolved `site-packages` directory if present.
    pub site_packages: Option<PathBuf>,
    /// Python version parsed directly from `pyvenv.cfg` in microseconds (zero subprocess overhead).
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchError {
    InvalidRegex(String),
    IoError(String),
}

// ============================================================================
// 1. FAST GITIGNORE-AWARE WORKSPACE INDEXER (`ignore`)
// ============================================================================

/// Fast `.gitignore`-aware polyglot directory indexer powered by `ignore`.
#[derive(Default)]
pub struct WorkspaceIndexer;

impl WorkspaceIndexer {
    pub fn new() -> Self {
        Self
    }

    /// Walks `root`, pruning `.git`, `__pycache__`, `.venv`, `.ruff_cache`, and `.gitignore` rules,
    /// returning deterministically sorted `WorkspaceFile` entries.
    pub fn index_directory(&self, root: &Path) -> Vec<WorkspaceFile> {
        let mut files = Vec::new();

        let walker = build_filtered_walker(root);
        for result in walker {
            let Ok(entry) = result else {
                continue;
            };
            if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                continue;
            }

            let path = entry.into_path();
            let relative_path = path
                .strip_prefix(root)
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|_| path.clone());
            let language = classify_file_language(&path);

            files.push(WorkspaceFile {
                path,
                rel_path: relative_path.clone(),
                relative_path,
                language,
            });
        }

        files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        files
    }

    /// Given a compiled `.pyd` or `.so` file in the workspace, collects its sibling files
    /// and delegates to `PydStubResolver` to locate `.pyi` stubs or `.pyx`/`.rs` native sources.
    pub fn resolve_binary_artifact(&self, binary_path: &Path) -> StubResolution {
        let siblings: Vec<PathBuf> = binary_path
            .parent()
            .and_then(|parent| fs::read_dir(parent).ok())
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| p != binary_path)
            .collect();

        let resolver = PydStubResolver::new();
        resolver.resolve_binary_module(binary_path, &siblings)
    }

    /// Recursively indexes the workspace into a `WorkspaceTreeNode` hierarchy respecting `.gitignore`
    /// and pruning development artifacts (`.git`, `.venv`, `__pycache__`, `target`, etc.).
    pub fn index_tree(&self, root: &Path) -> WorkspaceTreeNode {
        let root_name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace")
            .to_string();

        let mut root_node = WorkspaceTreeNode::new_dir(
            root.to_path_buf(),
            PathBuf::new(),
            root_name,
        );

        let walker = build_filtered_walker(root);
        for result in walker {
            let Ok(entry) = result else {
                continue;
            };

            if entry.depth() == 0 {
                continue;
            }

            let path = entry.into_path();
            let Ok(rel) = path.strip_prefix(root) else {
                continue;
            };

            let is_dir = path.is_dir();
            let components: Vec<std::ffi::OsString> = rel.iter().map(|c| c.to_os_string()).collect();
            if components.is_empty() {
                continue;
            }

            let mut curr = &mut root_node;
            let mut current_rel = PathBuf::new();
            let mut current_abs = root.to_path_buf();

            for (idx, comp) in components.iter().enumerate() {
                let is_last = idx == components.len() - 1;
                let name = comp.to_string_lossy().to_string();
                current_rel.push(comp);
                current_abs.push(comp);

                if is_last {
                    if is_dir {
                        if !curr.children.iter().any(|c| c.name == name && c.is_dir) {
                            curr.children.push(WorkspaceTreeNode::new_dir(
                                current_abs.clone(),
                                current_rel.clone(),
                                name,
                            ));
                        }
                    } else {
                        let lang = classify_file_language(&current_abs);
                        if !curr.children.iter().any(|c| c.name == name && !c.is_dir) {
                            curr.children.push(WorkspaceTreeNode::new_file(
                                current_abs.clone(),
                                current_rel.clone(),
                                name,
                                lang,
                            ));
                        }
                    }
                } else {
                    let pos = match curr.children.iter().position(|c| c.name == name && c.is_dir) {
                        Some(p) => p,
                        None => {
                            curr.children.push(WorkspaceTreeNode::new_dir(
                                current_abs.clone(),
                                current_rel.clone(),
                                name,
                            ));
                            curr.children.len() - 1
                        }
                    };
                    curr = &mut curr.children[pos];
                }
            }
        }

        root_node.sort_recursive();
        root_node
    }
}

fn build_filtered_walker(root: &Path) -> ignore::Walk {
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .require_git(false) // Ensure `.gitignore` is honored even if `.git` is absent
        .filter_entry(|entry: &DirEntry| !is_ignored_artifact_dir(entry));
    builder.build()
}

#[inline]
fn is_ignored_artifact_dir(entry: &DirEntry) -> bool {
    if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
        return false;
    }
    let Some(name) = entry.file_name().to_str() else {
        return false;
    };
    matches!(
        name,
        "__pycache__"
            | ".git"
            | ".venv"
            | "venv"
            | ".mypy_cache"
            | ".ruff_cache"
            | ".pytest_cache"
            | "node_modules"
            | "target"
    )
}

#[inline]
pub fn classify_file_language(path: &Path) -> Option<LanguageId> {
    let ext = path.extension()?.to_str()?;
    if ext.eq_ignore_ascii_case("py") || ext.eq_ignore_ascii_case("pyi") {
        Some(LanguageId::Python)
    } else if ext.eq_ignore_ascii_case("pyx") || ext.eq_ignore_ascii_case("pxd") {
        Some(LanguageId::Cython)
    } else if ext.eq_ignore_ascii_case("pyd") || ext.eq_ignore_ascii_case("so") {
        Some(LanguageId::PydBinaryStub)
    } else if ext.eq_ignore_ascii_case("rs") {
        Some(LanguageId::Rust)
    } else if ext.eq_ignore_ascii_case("c") || ext.eq_ignore_ascii_case("h") {
        Some(LanguageId::C)
    } else if ext.eq_ignore_ascii_case("cpp")
        || ext.eq_ignore_ascii_case("cc")
        || ext.eq_ignore_ascii_case("hpp")
    {
        Some(LanguageId::Cpp)
    } else {
        None
    }
}

// ============================================================================
// 2. RIPGREP (`grep-searcher` + `grep-regex`) WORKSPACE SEARCHER
// ============================================================================

/// Ripgrep-powered workspace regex searcher with automatic binary `.pyd`/`.pyc` skipping.
#[derive(Default)]
pub struct WorkspaceSearcher;

impl WorkspaceSearcher {
    pub fn new() -> Self {
        Self
    }

    /// Searches all non-ignored text files under `root` for `pattern`, capping results at `max_results`.
    pub fn search_regex(
        &self,
        root: &Path,
        pattern: &str,
        max_results: usize,
    ) -> Result<Vec<SearchMatch>, SearchError> {
        if max_results == 0 {
            return Ok(Vec::new());
        }

        let matcher =
            RegexMatcher::new(pattern).map_err(|e| SearchError::InvalidRegex(e.to_string()))?;

        // Configure ripgrep's searcher to immediately abort on binary files containing NUL (`\x00`) bytes
        let mut searcher = SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(b'\x00'))
            .line_number(true)
            .build();

        let mut matches = Vec::new();
        let walker = build_filtered_walker(root);

        for result in walker {
            if matches.len() >= max_results {
                break;
            }
            let Ok(entry) = result else {
                continue;
            };
            if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                continue;
            }

            let file_path = entry.into_path();
            // Skip known compiled binary extensions before opening the file descriptor
            if matches!(
                file_path.extension().and_then(|e| e.to_str()),
                Some("pyd" | "so" | "pyc" | "whl" | "exe" | "dll")
            ) {
                continue;
            }

            let relative_path = file_path
                .strip_prefix(root)
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|_| file_path.clone());

            let _ = searcher.search_path(
                &matcher,
                &file_path,
                UTF8(|line_num, line_text| {
                    let trimmed = line_text.trim_end_matches(&['\r', '\n'][..]);
                    let column = matcher
                        .find(trimmed.as_bytes())
                        .ok()
                        .flatten()
                        .map(|m| m.start())
                        .unwrap_or(0);

                    matches.push(SearchMatch {
                        path: file_path.clone(),
                        relative_path: relative_path.clone(),
                        line_number: line_num as usize,
                        column,
                        line_text: trimmed.to_string(),
                    });

                    // Return `Ok(false)` to stop scanning the current file once `max_results` is reached
                    Ok(matches.len() < max_results)
                }),
            );
        }

        Ok(matches)
    }
}

// ============================================================================
// 3. ZERO-SUBPROCESS PYTHON VIRTUALENV DETECTOR
// ============================================================================

/// Zero-subprocess Python virtual environment detector.
#[derive(Default)]
pub struct VirtualenvDetector;

impl VirtualenvDetector {
    pub fn new() -> Self {
        Self
    }

    /// Discovers a Python virtual environment by checking `active_env_override` (e.g. `$VIRTUAL_ENV`)
    /// first, then scanning `workspace_root` for `.venv`, `venv`, or `env`.
    pub fn detect(
        &self,
        workspace_root: &Path,
        active_env_override: Option<&Path>,
    ) -> Option<PythonEnvInfo> {
        if let Some(override_path) = active_env_override {
            if let Some(info) = Self::inspect_env_root(override_path) {
                return Some(info);
            }
        }

        for candidate_name in [".venv", "venv", "env", ".env"] {
            let candidate_root = workspace_root.join(candidate_name);
            if let Some(info) = Self::inspect_env_root(&candidate_root) {
                return Some(info);
            }
        }

        None
    }

    fn inspect_env_root(env_root: &Path) -> Option<PythonEnvInfo> {
        if !env_root.is_dir() {
            return None;
        }

        let interpreter_path = Self::find_interpreter_binary(env_root)?;
        let site_packages = Self::find_site_packages(env_root);
        let version = Self::parse_pyvenv_cfg_version(&env_root.join("pyvenv.cfg"));

        Some(PythonEnvInfo {
            root: env_root.to_path_buf(),
            interpreter_path,
            site_packages,
            version,
        })
    }

    fn find_interpreter_binary(env_root: &Path) -> Option<PathBuf> {
        let candidates = [
            env_root.join("Scripts").join("python.exe"),
            env_root.join("bin").join("python"),
            env_root.join("bin").join("python3"),
        ];
        candidates.into_iter().find(|p| p.is_file())
    }

    fn find_site_packages(env_root: &Path) -> Option<PathBuf> {
        // 1. Windows layout: `<venv>/Lib/site-packages`
        let win_sp = env_root.join("Lib").join("site-packages");
        if win_sp.is_dir() {
            return Some(win_sp);
        }

        // 2. Unix/macOS layout: `<venv>/lib/python3.x/site-packages`
        let unix_lib = env_root.join("lib");
        if let Ok(entries) = fs::read_dir(unix_lib) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let sp = path.join("site-packages");
                    if sp.is_dir() {
                        return Some(sp);
                    }
                }
            }
        }

        None
    }

    /// Reads `pyvenv.cfg` directly (usually <200 bytes) to extract `version` or `version_info`
    /// without spawning a slow `python --version` child process.
    fn parse_pyvenv_cfg_version(cfg_path: &Path) -> Option<String> {
        let content = fs::read_to_string(cfg_path).ok()?;
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some((key, val)) = trimmed.split_once('=') {
                let key = key.trim();
                if key == "version_info" || key == "version" {
                    let version_str = val.trim();
                    if !version_str.is_empty() {
                        return Some(version_str.to_string());
                    }
                }
            }
        }
        None
    }
}

// ============================================================================
// 4. NON-BLOCKING PYTEST RUNNER & SHORT-TRACEBACK PARSER
// ============================================================================

/// Command specification for launching `pytest` inside the discovered virtualenv.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PytestCommandSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
}

/// A structured test failure extracted from `pytest --tb=short -q` output.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PytestFailure {
    /// Full path to the failing `.py` file on disk.
    pub path: PathBuf,
    /// Workspace-relative path (e.g. `tests/test_pricing.py`).
    pub relative_path: PathBuf,
    /// 1-based line number of the failing assertion.
    pub line_number: usize,
    /// Name of the failing test function (e.g. `test_compute_discount`).
    pub test_name: String,
    /// Short error message (e.g. `AssertionError: assert 90.0 == 85.0`).
    pub message: String,
}

/// Summary report emitted when a `pytest` run finishes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PytestRunReport {
    pub passed: usize,
    pub failed: usize,
    pub failures: Vec<PytestFailure>,
    pub raw_output: String,
}

/// Real-time events streamed from the background `pytest` worker to the Floem UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PytestStreamEvent {
    OutputLine(String),
    Completed(PytestRunReport),
    Cancelled,
    Error(String),
}

/// Handle returned to the UI thread to read streaming `pytest` events or abort an in-flight test run.
pub struct PytestRunHandle {
    pub rx: mpsc::Receiver<PytestStreamEvent>,
    cancel_tx: watch::Sender<bool>,
}

impl PytestRunHandle {
    pub fn cancel(&self) {
        let _ = self.cancel_tx.send(true);
    }

    /// Returns a cloneable cancellation sender so the UI view-model can kill the child
    /// `pytest` process even after `self.rx` is moved into Floem's async event bridge.
    pub fn cancel_sender(&self) -> watch::Sender<bool> {
        self.cancel_tx.clone()
    }
}

/// Mockable trait for spawning `pytest` asynchronously.
pub trait PytestExecutor: Send + Sync + 'static {
    fn spawn_pytest(
        &self,
        spec: PytestCommandSpec,
        line_tx: mpsc::Sender<Result<String, String>>,
        cancel_rx: watch::Receiver<bool>,
    );
}

/// Builds a `PytestCommandSpec` using `python_env.interpreter_path` if available,
/// falling back to system `python` / `python3`.
pub fn build_pytest_command_spec(
    workspace_root: &Path,
    python_env: Option<&PythonEnvInfo>,
    target_file: Option<&Path>,
) -> PytestCommandSpec {
    let program = python_env
        .map(|env| env.interpreter_path.clone())
        .unwrap_or_else(|| {
            if cfg!(windows) {
                PathBuf::from("python")
            } else {
                PathBuf::from("python3")
            }
        });

    let mut args = vec![
        "-m".to_string(),
        "pytest".to_string(),
        "--tb=short".to_string(),
        "-q".to_string(),
        "--color=no".to_string(),
    ];

    if let Some(target) = target_file {
        let rel = target.strip_prefix(workspace_root).unwrap_or(target);
        args.push(rel.to_string_lossy().to_string());
    }

    PytestCommandSpec {
        program,
        args,
        working_dir: workspace_root.to_path_buf(),
    }
}

/// Parses `pytest --tb=short -q` output into passed/failed counts and clickable `PytestFailure` items.
pub fn parse_pytest_output(workspace_root: &Path, raw_output: &str) -> PytestRunReport {
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut failures: Vec<PytestFailure> = Vec::new();

    let mut current_section_test: Option<String> = None;
    let mut pending_location: Option<(PathBuf, usize, String)> = None;

    for raw_line in raw_output.lines() {
        let line = raw_line.trim_end_matches('\r');
        let trimmed = line.trim();

        // 1. Detect test failure section header: `_____ test_name _____`
        if trimmed.starts_with("___") && trimmed.ends_with("___") {
            let inner = trimmed.trim_matches('_').trim();
            if !inner.is_empty() {
                current_section_test = Some(inner.to_string());
            }
            continue;
        }

        // 2. Detect `--tb=short` location line: `tests/test_pricing.py:28: in test_compute_discount`
        if let Some((rel_path, line_num, fn_name)) = parse_tb_location_line(trimmed) {
            let test_name = current_section_test
                .clone()
                .unwrap_or_else(|| fn_name.to_string());
            pending_location = Some((rel_path, line_num, test_name));
            continue;
        }

        // 3. Detect `E   AssertionError: ...` line immediately following a traceback location
        if trimmed.starts_with("E   ") {
            let err_msg = trimmed.trim_start_matches("E   ").trim().to_string();
            if let Some((rel_path, line_num, test_name)) = pending_location.take() {
                let full_path = workspace_root.join(&rel_path);
                failures.push(PytestFailure {
                    path: full_path,
                    relative_path: rel_path,
                    line_number: line_num,
                    test_name,
                    message: err_msg,
                });
            }
            continue;
        }

        // 4. Parse summary counts: e.g. `2 failed, 3 passed in 0.12s` or `4 passed in 0.01s`
        if trimmed.contains(" in ") && (trimmed.contains("passed") || trimmed.contains("failed")) {
            let (p, f) = parse_summary_counts(trimmed);
            if p > 0 || f > 0 {
                passed = p;
                failed = f;
            }
        }
    }

    PytestRunReport {
        passed,
        failed,
        failures,
        raw_output: raw_output.to_string(),
    }
}

fn parse_tb_location_line(trimmed: &str) -> Option<(PathBuf, usize, &str)> {
    // Matches `<path>.py:<line>: in <func>`
    let (before_in, fn_name) = trimmed.split_once(": in ")?;
    let (file_part, line_part) = before_in.rsplit_once(':')?;
    if !file_part.ends_with(".py") {
        return None;
    }
    let line_num = line_part.trim().parse::<usize>().ok()?;
    Some((PathBuf::from(file_part.trim()), line_num, fn_name.trim()))
}

fn parse_summary_counts(summary_line: &str) -> (usize, usize) {
    let mut passed = 0usize;
    let mut failed = 0usize;

    let tokens: Vec<&str> = summary_line
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .collect();

    for window in tokens.windows(2) {
        if let Ok(count) = window[0].parse::<usize>() {
            if window[1] == "passed" {
                passed = count;
            } else if window[1] == "failed" || window[1] == "error" || window[1] == "errors" {
                failed += count;
            }
        }
    }

    (passed, failed)
}

/// Coordinates non-blocking `pytest` execution, real-time line streaming, and traceback parsing.
pub struct PytestRunner<E: PytestExecutor> {
    executor: Arc<E>,
}

impl<E: PytestExecutor> PytestRunner<E> {
    pub fn new(executor: E) -> Self {
        Self {
            executor: Arc::new(executor),
        }
    }

    pub fn start_run(&self, spec: PytestCommandSpec) -> PytestRunHandle {
        let (event_tx, event_rx) = mpsc::channel::<PytestStreamEvent>(128);
        let (line_tx, mut line_rx) = mpsc::channel::<Result<String, String>>(128);
        let (cancel_tx, mut cancel_rx) = watch::channel(false);

        let working_dir = spec.working_dir.clone();
        self.executor
            .spawn_pytest(spec, line_tx, cancel_rx.clone());

        tokio::spawn(async move {
            let mut accumulated = String::new();

            loop {
                tokio::select! {
                    biased;

                    changed = cancel_rx.changed() => {
                        if changed.is_ok() && *cancel_rx.borrow() {
                            let _ = event_tx.send(PytestStreamEvent::Cancelled).await;
                            return;
                        }
                    }

                    maybe_line = line_rx.recv() => {
                        match maybe_line {
                            Some(Ok(line)) => {
                                if *cancel_rx.borrow() {
                                    let _ = event_tx.send(PytestStreamEvent::Cancelled).await;
                                    return;
                                }
                                accumulated.push_str(&line);
                                accumulated.push('\n');
                                if event_tx.send(PytestStreamEvent::OutputLine(line)).await.is_err() {
                                    return;
                                }
                            }
                            Some(Err(err)) => {
                                let _ = event_tx.send(PytestStreamEvent::Error(err)).await;
                                return;
                            }
                            None => {
                                if *cancel_rx.borrow() {
                                    let _ = event_tx.send(PytestStreamEvent::Cancelled).await;
                                } else {
                                    let report = parse_pytest_output(&working_dir, &accumulated);
                                    let _ = event_tx.send(PytestStreamEvent::Completed(report)).await;
                                }
                                return;
                            }
                        }
                    }
                }
            }
        });

        PytestRunHandle {
            rx: event_rx,
            cancel_tx,
        }
    }
}

/// Production `PytestExecutor` backed by `tokio::process::Command` with `kill_on_drop(true)`.
#[derive(Default)]
pub struct ProcessPytestExecutor;

impl ProcessPytestExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl PytestExecutor for ProcessPytestExecutor {
    fn spawn_pytest(
        &self,
        spec: PytestCommandSpec,
        line_tx: mpsc::Sender<Result<String, String>>,
        mut cancel_rx: watch::Receiver<bool>,
    ) {
        tokio::spawn(async move {
            let mut cmd = TokioCommand::new(&spec.program);
            cmd.args(&spec.args)
                .current_dir(&spec.working_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(err) => {
                    let _ = line_tx
                        .send(Err(format!(
                            "Failed to spawn pytest ({}): {err}",
                            spec.program.display()
                        )))
                        .await;
                    return;
                }
            };

            let stdout = child.stdout.take().expect("stdout piped");
            let mut reader = BufReader::new(stdout).lines();

            loop {
                tokio::select! {
                    biased;

                    _ = cancel_rx.changed() => {
                        if *cancel_rx.borrow() {
                            let _ = child.kill().await;
                            return;
                        }
                    }

                    next_line = reader.next_line() => {
                        match next_line {
                            Ok(Some(line)) => {
                                if line_tx.send(Ok(line)).await.is_err() {
                                    let _ = child.kill().await;
                                    return;
                                }
                            }
                            Ok(None) => break,
                            Err(err) => {
                                let _ = line_tx.send(Err(err.to_string())).await;
                                break;
                            }
                        }
                    }
                }
            }

            let _ = child.wait().await;
        });
    }
}

// ============================================================================
// FILE & FOLDER OPERATIONS (HEADLESS & SAFE)
// ============================================================================

/// Creates a new empty file inside `parent_dir` with the given `file_name`.
/// Returns an error if `file_name` contains invalid path separators or if the file already exists.
pub fn create_file(parent_dir: &Path, file_name: &str) -> std::io::Result<PathBuf> {
    let trimmed = file_name.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed == "."
        || trimmed == ".."
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid file name",
        ));
    }

    let target = parent_dir.join(trimmed);
    if target.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("File already exists: {}", target.display()),
        ));
    }

    fs::File::create(&target)?;
    Ok(target)
}

/// Creates a new directory inside `parent_dir` with the given `folder_name`.
/// Returns an error if `folder_name` contains invalid characters or already exists.
pub fn create_folder(parent_dir: &Path, folder_name: &str) -> std::io::Result<PathBuf> {
    let trimmed = folder_name.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed == "."
        || trimmed == ".."
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid folder name",
        ));
    }

    let target = parent_dir.join(trimmed);
    if target.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("Directory already exists: {}", target.display()),
        ));
    }

    fs::create_dir_all(&target)?;
    Ok(target)
}

/// Deletes a file or directory at `path` from disk.
pub fn delete_entry(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Entry does not exist: {}", path.display()),
        ));
    }

    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// Builds the system file explorer command specification for revealing `path` in the OS file manager.
/// Pure and mockable for headless TDD verification.
pub fn build_explorer_command(path: &Path) -> (String, Vec<String>) {
    #[cfg(windows)]
    {
        if path.is_file() {
            (
                "explorer".to_string(),
                vec![format!("/select,{}", path.display())],
            )
        } else {
            (
                "explorer".to_string(),
                vec![path.to_string_lossy().to_string()],
            )
        }
    }

    #[cfg(target_os = "macos")]
    {
        if path.is_file() {
            (
                "open".to_string(),
                vec!["-R".to_string(), path.to_string_lossy().to_string()],
            )
        } else {
            ("open".to_string(), vec![path.to_string_lossy().to_string()])
        }
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let target = if path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        (
            "xdg-open".to_string(),
            vec![target.to_string_lossy().to_string()],
        )
    }
}

/// Spawns the native OS file explorer pointing to `path`.
pub fn reveal_in_system_explorer(path: &Path) -> std::io::Result<()> {
    let (prog, args) = build_explorer_command(path);
    std::process::Command::new(prog)
        .args(&args)
        .spawn()?;
    Ok(())
}

/// Builds the system command specification to open the native OS folder picker dialog.
/// Pure and mockable for headless TDD verification.
pub fn build_folder_picker_command() -> (String, Vec<String>) {
    #[cfg(windows)]
    {
        let script = concat!(
            "Add-Type -AssemblyName System.Windows.Forms; ",
            "$d = New-Object System.Windows.Forms.FolderBrowserDialog; ",
            "$d.Description = 'Select Project Folder for Lide'; ",
            "$top = New-Object System.Windows.Forms.Form -Property @{TopMost=$true}; ",
            "if ($d.ShowDialog($top) -eq [System.Windows.Forms.DialogResult]::OK) { ",
            "  [Console]::Out.Write($d.SelectedPath) ",
            "}"
        );
        (
            "powershell".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-Sta".to_string(),
                "-Command".to_string(),
                script.to_string(),
            ],
        )
    }

    #[cfg(target_os = "macos")]
    {
        (
            "osascript".to_string(),
            vec![
                "-e".to_string(),
                "POSIX path of (choose folder with prompt \"Select Project Folder for Lide\")".to_string(),
            ],
        )
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    {
        (
            "zenity".to_string(),
            vec![
                "--file-selection".to_string(),
                "--directory".to_string(),
                "--title=Select Project Folder for Lide".to_string(),
            ],
        )
    }
}

/// Spawns the native OS folder picker dialog and waits synchronously for the user's selection.
/// Intended to be run in a background worker via `tokio::task::spawn_blocking`.
pub fn spawn_native_folder_picker() -> Option<PathBuf> {
    let (prog, args) = build_folder_picker_command();

    #[cfg(windows)]
    let prog_to_run = {
        if let Ok(sys_root) = std::env::var("SystemRoot") {
            let ps_full = PathBuf::from(&sys_root)
                .join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe");
            if ps_full.exists() {
                ps_full.to_string_lossy().to_string()
            } else {
                prog.clone()
            }
        } else {
            prog.clone()
        }
    };

    #[cfg(not(windows))]
    let prog_to_run = prog;

    let mut cmd = std::process::Command::new(&prog_to_run);
    cmd.args(&args);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    if let Ok(output) = cmd.output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !stdout.is_empty() {
                let path = PathBuf::from(stdout);
                if path.is_dir() {
                    return Some(path);
                }
            }
        }
    }

    // Windows Fallback: Try VBScript Shell.Application if PowerShell was blocked or failed
    #[cfg(windows)]
    {
        if let Some(path) = spawn_vbscript_folder_picker() {
            return Some(path);
        }
    }

    None
}

#[cfg(windows)]
fn spawn_vbscript_folder_picker() -> Option<PathBuf> {
    let script = concat!(
        "Set a = CreateObject(\"Shell.Application\")\r\n",
        "Set f = a.BrowseForFolder(0, \"Select Project Folder for Lide\", 0, 0)\r\n",
        "If Not f Is Nothing Then WScript.StdOut.Write f.Self.Path\r\n"
    );
    let temp_vbs = std::env::temp_dir().join("lide_folder_picker.vbs");
    if std::fs::write(&temp_vbs, script).is_err() {
        return None;
    }

    let cscript_path = if let Ok(sys_root) = std::env::var("SystemRoot") {
        PathBuf::from(sys_root).join("System32").join("cscript.exe")
    } else {
        PathBuf::from("cscript.exe")
    };

    let mut cmd = std::process::Command::new(cscript_path);
    cmd.arg("//nologo").arg(&temp_vbs);

    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().ok();
    let _ = std::fs::remove_file(temp_vbs);

    if let Some(out) = output {
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !stdout.is_empty() {
                let path = PathBuf::from(stdout);
                if path.is_dir() {
                    return Some(path);
                }
            }
        }
    }
    None
}
