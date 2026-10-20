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
    /// Detected language (Python, Cython, PydBinaryStub, C, Cpp, Rust).
    pub language: Option<LanguageId>,
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
