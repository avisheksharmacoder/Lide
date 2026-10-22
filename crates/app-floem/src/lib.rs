use std::collections::HashMap;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

use ai_engine::{
    AiEngine, AiPromptRequest, AiStreamEvent, AiStreamHandle, AiTaskKind, AstPromptBuilder,
    LlmStreamingClient,
};
use editor_core::{Cursor, MoveDirection, Position, TextBuffer, VisibleLine};
use floem::reactive::{RwSignal, Scope, SignalGet, SignalUpdate};
use lang_contract::{
    AstAnalyzer, CodeFormatter, Diagnostic, DiagnosticSeverity, EnclosingContext, FormatError,
    HighlightSpan, HighlightToken, LanguageId, OutlineSymbol, StubResolution, SymbolKind,
    SyntaxHighlighter,
};
use lang_python::{PythonAstAnalyzer, PythonFormatter, PythonSyntaxHighlighter};
use workspace_engine::{
    build_pytest_command_spec, classify_file_language, PythonEnvInfo, PytestCommandSpec,
    PytestExecutor, PytestFailure, PytestRunHandle, PytestRunner, PytestStreamEvent, SearchError,
    SearchMatch, VirtualenvDetector, WorkspaceFile, WorkspaceIndexer, WorkspaceSearcher,
};

pub mod bundled_fonts;

/// Active view mode for the left sidebar panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SidebarTab {
    ProjectFiles,
    Structure,
}

/// Active floating dropdown menu in the top toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ActiveMenu {
    #[default]
    None,
    File,
    Settings,
}

/// A line-local colored text span ready for Floem text layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledTextSegment {
    /// Byte range relative to `RenderedLine::text` (`0..line.text.len()`).
    pub rel_byte_range: Range<usize>,
    pub token: Option<HighlightToken>,
}

/// Self-contained view model for a single visible row inside Floem's virtualized viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedLine {
    /// Zero-based document line index.
    pub line_idx: usize,
    /// One-based line number displayed in the left gutter (`line_idx + 1`).
    pub gutter_number: usize,
    /// True if the primary cursor currently sits on this line.
    pub is_active_line: bool,
    /// Column offset of the cursor if active on this line.
    pub cursor_col: Option<usize>,
    /// Highest severity diagnostic on this line (Error > Warning > Info > Hint) for gutter badges.
    pub highest_diagnostic: Option<DiagnosticSeverity>,
    /// Line text stripped of trailing CR/LF.
    pub text: String,
    /// Contiguous syntax-highlighted segments covering `text`.
    pub styled_segments: Vec<StyledTextSegment>,
}

/// Flattened representation of an `OutlineSymbol` for the PyCharm-style Structure sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FlatOutlineItem {
    pub name: String,
    pub kind: SymbolKind,
    /// Nesting depth (`0` for top-level classes/functions, `1` for class methods).
    pub depth: usize,
    /// Zero-based line index to jump to when clicked in the Structure sidebar.
    pub start_line: usize,
    pub detail: Option<String>,
}

/// Unidirectional commands dispatched by keyboard shortcuts, mouse clicks, or background workers.
#[derive(Debug, Clone, PartialEq)]
pub enum EditorCommand {
    /// Replaces the buffer with new Python source text and runs initial viewport parse.
    LoadSource(String),
    /// Indexes a workspace root directory and detects its Python virtual environment.
    OpenWorkspace(PathBuf),
    /// Opens a file from the workspace file tree (intercepting `.pyd`/`.so` binaries to stubs).
    OpenWorkspaceFile(PathBuf),
    /// Switches the left sidebar between Project Files and AST Structure.
    SwitchSidebarTab(SidebarTab),
    /// Updates the vertical scroll offset (in pixels) and viewport height (in pixels).
    ScrollToPixel {
        offset_y: f64,
        viewport_height: f64,
    },
    /// Inserts text at the current cursor position and incrementally updates highlights.
    InsertText(String),
    /// Deletes one grapheme cluster backward.
    Backspace,
    /// Moves the cursor in the given direction and auto-scrolls if the cursor leaves the viewport.
    MoveCursor(MoveDirection),
    /// Sets the cursor directly (e.g. clicking a specific line/column in the viewport).
    SetCursor(Position),
    /// Triggered when a user clicks a class or method in the left Structure sidebar.
    ClickStructureSymbol {
        start_line: usize,
    },
    /// Runs the ephemeral Ruff AST pass to refresh the Structure sidebar and gutter diagnostics.
    RunEphemeralAnalysis,
    /// Formats the active Python buffer in memory using `ruff_python_formatter` (Ctrl+S / Cmd+S).
    FormatDocument,
    /// Reverts the last edit, formatting pass, or AI refactor (Ctrl+Z / Cmd+Z).
    Undo,
    /// Re-applies the last undone edit, formatting pass, or AI refactor (Ctrl+Shift+Z / Ctrl+Y).
    Redo,
    /// Toggles the right-hand AI Assistant split pane open or closed.
    ToggleAiPane,
    /// Slices the enclosing Python AST function/class around the cursor and prepares an AI task.
    TriggerAiTask(AiTaskKind),
    /// Feeds a streaming token, completion, or error event from `ai-engine` into Floem signals.
    ReceiveAiStreamEvent(AiStreamEvent),
    /// Cancels an in-flight `ollama-rs` token stream.
    CancelAiStream,
    /// Replaces the active AST symbol's line range in the buffer with `ai_clean_code`.
    ApplyAiSuggestion,
    /// Toggles the bottom Find-in-Files regex search panel open or closed (Ctrl+Shift+F).
    ToggleSearchPanel,
    /// Updates the active regex pattern string for workspace search.
    SetSearchQuery(String),
    /// Runs `WorkspaceSearcher::search_regex` across the open workspace root.
    ExecuteWorkspaceSearch {
        max_results: usize,
    },
    /// Opens the matched file (if not already open), moves the cursor to `(line - 1, col)`,
    /// and scrolls the virtualized viewport so the match is visible.
    ClickSearchResult(SearchMatch),
    /// Toggles the bottom Pytest Runner panel open or closed (Ctrl+Shift+T).
    TogglePytestPanel,
    /// Prepares a non-blocking `pytest` run (either for `target_file` or the full workspace).
    TriggerPytestRun {
        target_file: Option<PathBuf>,
    },
    /// Feeds a streaming line, final `PytestRunReport`, or error into Floem signals.
    ReceivePytestEvent(PytestStreamEvent),
    /// Aborts an in-flight `pytest` child process.
    CancelPytestRun,
    /// Opens the failing test file, moves the cursor to `(line_number - 1, 0)`,
    /// and scrolls the virtualized viewport so the failed assertion is visible.
    ClickPytestFailure(PytestFailure),
    /// Inserts a newline with Python smart auto-indentation (`Enter`).
    InsertNewlineAutoIndent,
    /// Indents the active line by 4 PEP 8 spaces (`Tab`).
    IndentLine,
    /// Unindents the active line by up to 4 leading spaces (`Shift+Tab`).
    DedentLine,
    /// Formats the active buffer with Ruff (if valid Python), writes atomically to `active_file_path`,
    /// clears `is_dirty`, and refreshes the AST outline (`Ctrl+S` / `Cmd+S`).
    SaveActiveFile,
    /// Toggles the specified top toolbar dropdown menu (`File` or `Settings`).
    ToggleMenu(ActiveMenu),
    /// Closes any open top toolbar dropdown menu.
    CloseMenu,
    /// Clears the active buffer for a new blank document.
    NewDocument,
    /// Opens or closes the Font Settings modal dialog.
    ToggleFontDialog,
    /// Sets the UI / IDE font family.
    SetIdeFont(String),
    /// Sets the Code Editor font family.
    SetEditorFont(String),
    /// Sets the Terminal font family.
    SetTerminalFont(String),
}

/// Granular Floem reactive signals. Splitting these ensures scrolling, typing, or toggling
/// sidebars never forces unaffected panels to re-render.
#[derive(Clone, Copy)]
pub struct EditorSignals {
    /// Strictly the lines currently visible inside the scroll viewport.
    pub visible_lines: RwSignal<Vec<RenderedLine>>,
    /// Current cursor coordinates.
    pub cursor: RwSignal<Cursor>,
    /// Flattened AST symbols for the left Structure sidebar.
    pub outline_items: RwSignal<Vec<FlatOutlineItem>>,
    /// Indexed files in the current workspace for the Project Files sidebar.
    pub workspace_files: RwSignal<Vec<WorkspaceFile>>,
    /// Root path of the currently opened workspace.
    pub workspace_root: RwSignal<Option<PathBuf>>,
    /// Currently selected tab in the left sidebar (`ProjectFiles` or `Structure`).
    pub active_sidebar_tab: RwSignal<SidebarTab>,
    /// Path of the currently open document (or resolved `.pyi`/`.pyx` stub).
    pub active_file_path: RwSignal<Option<PathBuf>>,
    /// Detected Python virtual environment metadata for the workspace.
    pub python_env: RwSignal<Option<PythonEnvInfo>>,
    /// Active Ruff & syntax diagnostics for the bottom Problems panel and gutter.
    pub diagnostics: RwSignal<Vec<Diagnostic>>,
    /// Formatted status bar string including cursor coordinates, diagnostics, and Python venv version.
    pub status_bar_text: RwSignal<String>,
    /// Current vertical pixel scroll offset.
    pub scroll_offset_y: RwSignal<f64>,
    /// Total virtual scrollable height in pixels (`total_lines * line_height_px`).
    pub total_content_height: RwSignal<f64>,
    /// Holds `Some(error_message)` if the last `FormatDocument` failed due to a SyntaxError.
    pub last_format_error: RwSignal<Option<String>>,
    /// Whether the right-hand AI Assistant split pane is currently visible.
    pub ai_pane_open: RwSignal<bool>,
    /// True while tokens are actively streaming from `ai-engine`.
    pub ai_is_streaming: RwSignal<bool>,
    /// Enclosing Python function/class sliced by the AST for the current AI task.
    pub active_ai_context: RwSignal<Option<EnclosingContext>>,
    /// Live accumulated token text displayed in the right-hand AI pane.
    pub ai_stream_text: RwSignal<String>,
    /// Markdown-stripped Python code ready for one-click buffer replacement.
    pub ai_clean_code: RwSignal<Option<String>>,
    /// Selected local Ollama model identifier (e.g. `"qwen2.5-coder:7b"`).
    pub ai_model_name: RwSignal<String>,
    /// Whether the bottom Find-in-Files regex search panel is currently open.
    pub search_panel_open: RwSignal<bool>,
    /// Current regex pattern in the Find-in-Files search box.
    pub search_query: RwSignal<String>,
    /// Matched lines returned by `WorkspaceSearcher::search_regex`.
    pub search_results: RwSignal<Vec<SearchMatch>>,
    /// Holds `Some(error)` if the user entered an invalid regex pattern.
    pub search_error: RwSignal<Option<String>>,
    /// Whether the bottom Pytest Runner panel is currently visible.
    pub pytest_panel_open: RwSignal<bool>,
    /// True while a background `pytest` run is actively streaming.
    pub pytest_is_running: RwSignal<bool>,
    /// Streamed stdout lines from the current or most recent `pytest` execution.
    pub pytest_output_lines: RwSignal<Vec<String>>,
    /// Clickable test failures parsed from `--tb=short` output.
    pub pytest_failures: RwSignal<Vec<PytestFailure>>,
    /// Formatted test summary badge (e.g. `"1 failed, 3 passed"`).
    pub pytest_summary: RwSignal<Option<String>>,
    /// True if the buffer has unsaved modifications since the last load or save.
    pub is_dirty: RwSignal<bool>,
    /// Currently open dropdown menu in the top toolbar.
    pub active_menu: RwSignal<ActiveMenu>,
    /// Whether the Font Settings modal dialog is currently open.
    pub font_dialog_open: RwSignal<bool>,
    /// Currently selected IDE / UI font family.
    pub ide_font: RwSignal<String>,
    /// Currently selected Code Editor font family.
    pub editor_font: RwSignal<String>,
    /// Currently selected Terminal font family.
    pub terminal_font: RwSignal<String>,
}

/// Headless controller that owns the Rope buffer, Tree-sitter highlighter, Ruff analyzer,
/// Ruff formatter, Workspace indexer/searcher/virtualenv detector, AI prompt builder, and Floem signals.
pub struct EditorViewModel {
    scope: Scope,
    buffer: TextBuffer,
    source_cache: String,
    highlighter: PythonSyntaxHighlighter,
    analyzer: PythonAstAnalyzer,
    formatter: PythonFormatter,
    indexer: WorkspaceIndexer,
    searcher: WorkspaceSearcher,
    venv_detector: VirtualenvDetector,
    prompt_builder: AstPromptBuilder,
    active_ai_cancel: Option<tokio::sync::watch::Sender<bool>>,
    active_pytest_cancel: Option<tokio::sync::watch::Sender<bool>>,
    line_diagnostics: HashMap<usize, DiagnosticSeverity>,
    line_height_px: f64,
    viewport_height_px: f64,
    pub signals: EditorSignals,
}

impl EditorViewModel {
    /// Creates a new reactive editor view model with the given row height and initial viewport height.
    pub fn new(line_height_px: f64, viewport_height_px: f64) -> Self {
        let scope = Scope::new();
        let line_height = line_height_px.max(1.0);
        let viewport_height = viewport_height_px.max(line_height);

        let buffer = TextBuffer::new();
        let initial_cursor = buffer.cursor();

        let signals = EditorSignals {
            visible_lines: scope.create_rw_signal(Vec::new()),
            cursor: scope.create_rw_signal(initial_cursor),
            outline_items: scope.create_rw_signal(Vec::new()),
            workspace_files: scope.create_rw_signal(Vec::new()),
            workspace_root: scope.create_rw_signal(None),
            active_sidebar_tab: scope.create_rw_signal(SidebarTab::ProjectFiles),
            active_file_path: scope.create_rw_signal(None),
            python_env: scope.create_rw_signal(None),
            diagnostics: scope.create_rw_signal(Vec::new()),
            status_bar_text: scope.create_rw_signal(Self::format_status(
                initial_cursor.head,
                buffer.len_lines(),
                0,
                None,
                None,
                false,
            )),
            scroll_offset_y: scope.create_rw_signal(0.0),
            total_content_height: scope
                .create_rw_signal((buffer.len_lines() as f64) * line_height),
            last_format_error: scope.create_rw_signal(None),
            ai_pane_open: scope.create_rw_signal(false),
            ai_is_streaming: scope.create_rw_signal(false),
            active_ai_context: scope.create_rw_signal(None),
            ai_stream_text: scope.create_rw_signal(String::new()),
            ai_clean_code: scope.create_rw_signal(None),
            ai_model_name: scope.create_rw_signal("qwen2.5-coder:7b".to_string()),
            search_panel_open: scope.create_rw_signal(false),
            search_query: scope.create_rw_signal(String::new()),
            search_results: scope.create_rw_signal(Vec::new()),
            search_error: scope.create_rw_signal(None),
            pytest_panel_open: scope.create_rw_signal(false),
            pytest_is_running: scope.create_rw_signal(false),
            pytest_output_lines: scope.create_rw_signal(Vec::new()),
            pytest_failures: scope.create_rw_signal(Vec::new()),
            pytest_summary: scope.create_rw_signal(None),
            is_dirty: scope.create_rw_signal(false),
            active_menu: scope.create_rw_signal(ActiveMenu::None),
            font_dialog_open: scope.create_rw_signal(false),
            ide_font: scope.create_rw_signal("Quicksand".to_string()),
            editor_font: scope.create_rw_signal("JetBrains Mono".to_string()),
            terminal_font: scope.create_rw_signal("JetBrains Mono".to_string()),
        };

        let mut vm = Self {
            scope,
            buffer,
            source_cache: String::new(),
            highlighter: PythonSyntaxHighlighter::new(),
            analyzer: PythonAstAnalyzer::new(),
            formatter: PythonFormatter::new(),
            indexer: WorkspaceIndexer::new(),
            searcher: WorkspaceSearcher::new(),
            venv_detector: VirtualenvDetector::new(),
            prompt_builder: AstPromptBuilder::new(),
            active_ai_cancel: None,
            active_pytest_cancel: None,
            line_diagnostics: HashMap::new(),
            line_height_px: line_height,
            viewport_height_px: viewport_height,
            signals,
        };

        vm.highlighter.parse_full("");
        vm.refresh_visible_viewport();
        vm
    }

    /// Builds the virtualenv-aware `PytestCommandSpec`, resets the test panel signals,
    /// and starts a non-blocking run on the provided `PytestRunner<E>`.
    pub fn start_pytest_with_runner<E: PytestExecutor>(
        &mut self,
        target_file: Option<PathBuf>,
        runner: &PytestRunner<E>,
    ) -> Option<PytestRunHandle> {
        let spec = self.prepare_pytest_spec(target_file.as_deref())?;
        let handle = runner.start_run(spec);
        self.active_pytest_cancel = Some(handle.cancel_sender());
        Some(handle)
    }

    fn prepare_pytest_spec(&mut self, target_file: Option<&Path>) -> Option<PytestCommandSpec> {
        if let Some(cancel_tx) = self.active_pytest_cancel.take() {
            let _ = cancel_tx.send(true);
        }

        if !self.signals.pytest_panel_open.get_untracked() {
            self.signals.pytest_panel_open.set(true);
        }

        self.signals.pytest_output_lines.set(Vec::new());
        self.signals.pytest_failures.set(Vec::new());
        self.signals
            .pytest_summary
            .set(Some("Running pytest...".to_string()));
        self.signals.pytest_is_running.set(true);

        let root = self
            .signals
            .workspace_root
            .get_untracked()
            .unwrap_or_else(|| PathBuf::from("."));
        let py_env = self.signals.python_env.get_untracked();

        Some(build_pytest_command_spec(
            &root,
            py_env.as_ref(),
            target_file,
        ))
    }

    /// Slices the AST around the current cursor, updates the AI pane signals, and starts a stream
    /// on the provided `AiEngine<C>`.
    pub fn start_ai_task_with_engine<C: LlmStreamingClient>(
        &mut self,
        task: AiTaskKind,
        engine: &AiEngine<C>,
    ) -> Option<AiStreamHandle> {
        let req = self.prepare_ai_task_request(task)?;
        let handle = engine.start_stream(req);
        self.active_ai_cancel = Some(handle.cancel_sender());
        Some(handle)
    }

    /// Dispatches an `EditorCommand`, mutating internal state and notifying only affected signals.
    pub fn dispatch(&mut self, cmd: EditorCommand) {
        match cmd {
            EditorCommand::LoadSource(source) => {
                self.load_buffer_text(source);
            }

            EditorCommand::OpenWorkspace(root_path) => {
                let files = self.indexer.index_directory(&root_path);
                let env_override = std::env::var_os("VIRTUAL_ENV").map(PathBuf::from);
                let detected_env = self
                    .venv_detector
                    .detect(&root_path, env_override.as_deref());

                self.signals.workspace_root.set(Some(root_path));
                self.signals.workspace_files.set(files);
                self.signals.python_env.set(detected_env);
                self.update_status_bar_signal();
            }

            EditorCommand::OpenWorkspaceFile(path) => {
                let (resolved_path, content) = self.resolve_and_read_file(&path);
                self.signals.active_file_path.set(Some(resolved_path));
                self.load_buffer_text(content);
                self.run_ephemeral_analysis_internal();
            }

            EditorCommand::SwitchSidebarTab(tab) => {
                if self.signals.active_sidebar_tab.get_untracked() != tab {
                    self.signals.active_sidebar_tab.set(tab);
                }
            }

            EditorCommand::ScrollToPixel {
                offset_y,
                viewport_height,
            } => {
                self.viewport_height_px = viewport_height.max(self.line_height_px);
                let max_scroll = ((self.buffer.len_lines() as f64) * self.line_height_px
                    - self.viewport_height_px)
                    .max(0.0);
                let clamped_offset = offset_y.clamp(0.0, max_scroll);

                self.signals.scroll_offset_y.set(clamped_offset);
                self.refresh_visible_viewport();
            }

            EditorCommand::InsertText(text) => {
                if text.is_empty() {
                    return;
                }
                self.buffer.insert_at_cursor(&text);
                self.signals.is_dirty.set(true);
                self.sync_after_buffer_mutation();
            }

            EditorCommand::InsertNewlineAutoIndent => {
                self.buffer.insert_newline_auto_indent();
                self.signals.is_dirty.set(true);
                self.sync_after_buffer_mutation();
            }

            EditorCommand::IndentLine => {
                self.buffer.indent_line();
                self.signals.is_dirty.set(true);
                self.sync_after_buffer_mutation();
            }

            EditorCommand::DedentLine => {
                let prev_bytes = self.buffer.len_bytes();
                self.buffer.dedent_line();
                if self.buffer.len_bytes() != prev_bytes {
                    self.signals.is_dirty.set(true);
                    self.sync_after_buffer_mutation();
                }
            }

            EditorCommand::Backspace => {
                let prev_bytes = self.buffer.len_bytes();
                self.buffer.delete_backward();
                if self.buffer.len_bytes() != prev_bytes {
                    self.signals.is_dirty.set(true);
                }
                self.sync_after_buffer_mutation();
            }

            EditorCommand::MoveCursor(dir) => {
                self.buffer.move_cursor(dir);
                self.signals.cursor.set(self.buffer.cursor());
                self.ensure_cursor_visible();
                self.update_status_bar_signal();
                self.refresh_visible_viewport();
            }

            EditorCommand::SetCursor(pos) => {
                self.buffer.set_cursor(pos);
                self.signals.cursor.set(self.buffer.cursor());
                self.ensure_cursor_visible();
                self.update_status_bar_signal();
                self.refresh_visible_viewport();
            }

            EditorCommand::ClickStructureSymbol { start_line } => {
                self.buffer.set_cursor(Position::new(start_line, 0));
                self.signals.cursor.set(self.buffer.cursor());

                let target_px = (start_line as f64) * self.line_height_px;
                let max_scroll = ((self.buffer.len_lines() as f64) * self.line_height_px
                    - self.viewport_height_px)
                    .max(0.0);
                self.signals
                    .scroll_offset_y
                    .set(target_px.clamp(0.0, max_scroll));

                self.update_status_bar_signal();
                self.refresh_visible_viewport();
            }

            EditorCommand::RunEphemeralAnalysis => {
                self.run_ephemeral_analysis_internal();
            }

            EditorCommand::FormatDocument => {
                self.format_buffer_in_memory();
            }

            EditorCommand::SaveActiveFile => {
                // 1. Format in memory via Ruff first (if syntax is valid)
                self.format_buffer_in_memory();

                // 2. Write atomically to disk if an active file path is bound
                if let Some(target_path) = self.signals.active_file_path.get_untracked() {
                    let temp_path = target_path.with_extension("pyrust.tmp");
                    if fs::write(&temp_path, &self.source_cache).is_ok() {
                        let _ = fs::rename(&temp_path, &target_path)
                            .or_else(|_| fs::write(&target_path, &self.source_cache));
                    } else {
                        let _ = fs::write(&target_path, &self.source_cache);
                    }
                }

                self.signals.is_dirty.set(false);
                self.run_ephemeral_analysis_internal();
            }

            EditorCommand::Undo => {
                if self.buffer.undo() {
                    self.signals.last_format_error.set(None);
                    self.sync_after_buffer_mutation();
                }
            }

            EditorCommand::Redo => {
                if self.buffer.redo() {
                    self.signals.last_format_error.set(None);
                    self.sync_after_buffer_mutation();
                }
            }

            EditorCommand::ToggleAiPane => {
                let currently_open = self.signals.ai_pane_open.get_untracked();
                self.signals.ai_pane_open.set(!currently_open);
            }

            EditorCommand::TriggerAiTask(task) => {
                let _ = self.prepare_ai_task_request(task);
            }

            EditorCommand::ReceiveAiStreamEvent(event) => match event {
                AiStreamEvent::Token(chunk) => {
                    self.signals.ai_stream_text.update(|text| text.push_str(&chunk));
                }
                AiStreamEvent::Completed {
                    raw_response,
                    clean_python_code,
                } => {
                    self.signals.ai_stream_text.set(raw_response);
                    self.signals.ai_clean_code.set(Some(clean_python_code));
                    self.signals.ai_is_streaming.set(false);
                }
                AiStreamEvent::Cancelled => {
                    self.signals.ai_is_streaming.set(false);
                }
                AiStreamEvent::Error(err) => {
                    self.signals.ai_stream_text.set(format!("# AI Error:\n# {err}"));
                    self.signals.ai_is_streaming.set(false);
                }
            },

            EditorCommand::CancelAiStream => {
                if let Some(cancel_tx) = self.active_ai_cancel.take() {
                    let _ = cancel_tx.send(true);
                }
                self.signals.ai_is_streaming.set(false);
            }

            EditorCommand::ApplyAiSuggestion => {
                let maybe_ctx = self.signals.active_ai_context.get_untracked();
                let maybe_code = self.signals.ai_clean_code.get_untracked();

                if let (Some(ctx), Some(clean_code)) = (maybe_ctx, maybe_code) {
                    if self
                        .buffer
                        .replace_line_range(ctx.start_line, ctx.end_line, &clean_code)
                    {
                        self.signals.is_dirty.set(true);
                        self.sync_after_buffer_mutation();
                        self.run_ephemeral_analysis_internal();
                    }
                }
            }

            EditorCommand::ToggleSearchPanel => {
                let is_open = self.signals.search_panel_open.get_untracked();
                self.signals.search_panel_open.set(!is_open);
            }

            EditorCommand::SetSearchQuery(query) => {
                self.signals.search_query.set(query);
            }

            EditorCommand::ExecuteWorkspaceSearch { max_results } => {
                let query = self.signals.search_query.get_untracked();
                if query.trim().is_empty() {
                    self.signals.search_results.set(Vec::new());
                    self.signals.search_error.set(None);
                    return;
                }

                let Some(root) = self.signals.workspace_root.get_untracked() else {
                    self.signals.search_results.set(Vec::new());
                    self.signals
                        .search_error
                        .set(Some("No workspace root open".to_string()));
                    return;
                };

                match self.searcher.search_regex(&root, &query, max_results) {
                    Ok(matches) => {
                        self.signals.search_error.set(None);
                        self.signals.search_results.set(matches);
                    }
                    Err(SearchError::InvalidRegex(err) | SearchError::IoError(err)) => {
                        self.signals.search_results.set(Vec::new());
                        self.signals.search_error.set(Some(err));
                    }
                }
            }

            EditorCommand::ClickSearchResult(hit) => {
                let needs_file_load = self
                    .signals
                    .active_file_path
                    .get_untracked()
                    .as_ref()
                    != Some(&hit.path);

                if needs_file_load {
                    let (resolved_path, content) = self.resolve_and_read_file(&hit.path);
                    self.signals.active_file_path.set(Some(resolved_path));
                    self.load_buffer_text(content);
                    self.run_ephemeral_analysis_internal();
                }

                let target_line = hit.line_number.saturating_sub(1);
                let target_pos = Position::new(target_line, hit.column);
                self.buffer.set_cursor(target_pos);
                self.signals.cursor.set(self.buffer.cursor());

                self.ensure_cursor_visible();
                self.update_status_bar_signal();
                self.refresh_visible_viewport();
            }

            EditorCommand::TogglePytestPanel => {
                let open = self.signals.pytest_panel_open.get_untracked();
                self.signals.pytest_panel_open.set(!open);
            }

            EditorCommand::TriggerPytestRun { target_file } => {
                let _ = self.prepare_pytest_spec(target_file.as_deref());
            }

            EditorCommand::ReceivePytestEvent(event) => match event {
                PytestStreamEvent::OutputLine(line) => {
                    self.signals
                        .pytest_output_lines
                        .update(|lines| lines.push(line));
                }
                PytestStreamEvent::Completed(report) => {
                    let summary = format!("{} failed, {} passed", report.failed, report.passed);
                    self.signals.pytest_summary.set(Some(summary));
                    self.signals.pytest_failures.set(report.failures);
                    self.signals.pytest_is_running.set(false);
                }
                PytestStreamEvent::Cancelled => {
                    self.signals
                        .pytest_summary
                        .set(Some("Pytest cancelled".to_string()));
                    self.signals.pytest_is_running.set(false);
                }
                PytestStreamEvent::Error(err) => {
                    self.signals
                        .pytest_summary
                        .set(Some(format!("Pytest error: {err}")));
                    self.signals.pytest_is_running.set(false);
                }
            },

            EditorCommand::CancelPytestRun => {
                if let Some(cancel_tx) = self.active_pytest_cancel.take() {
                    let _ = cancel_tx.send(true);
                }
                self.signals.pytest_is_running.set(false);
            }

            EditorCommand::ClickPytestFailure(failure) => {
                let needs_load = self.signals.active_file_path.get_untracked().as_ref()
                    != Some(&failure.path);

                if needs_load {
                    let (resolved_path, content) = self.resolve_and_read_file(&failure.path);
                    self.signals.active_file_path.set(Some(resolved_path));
                    self.load_buffer_text(content);
                    self.run_ephemeral_analysis_internal();
                }

                let target_line = failure.line_number.saturating_sub(1);
                self.buffer.set_cursor(Position::new(target_line, 0));
                self.signals.cursor.set(self.buffer.cursor());

                self.ensure_cursor_visible();
                self.update_status_bar_signal();
                self.refresh_visible_viewport();
            }

            EditorCommand::ToggleMenu(menu) => {
                let current = self.signals.active_menu.get_untracked();
                if current == menu {
                    self.signals.active_menu.set(ActiveMenu::None);
                } else {
                    self.signals.active_menu.set(menu);
                }
            }

            EditorCommand::CloseMenu => {
                self.signals.active_menu.set(ActiveMenu::None);
            }

            EditorCommand::NewDocument => {
                self.load_buffer_text(String::new());
                self.signals.active_file_path.set(None);
                self.signals.is_dirty.set(false);
                self.signals.active_menu.set(ActiveMenu::None);
                self.run_ephemeral_analysis_internal();
            }

            EditorCommand::ToggleFontDialog => {
                let current = self.signals.font_dialog_open.get_untracked();
                self.signals.font_dialog_open.set(!current);
                self.signals.active_menu.set(ActiveMenu::None);
            }

            EditorCommand::SetIdeFont(font) => {
                self.signals.ide_font.set(font);
            }

            EditorCommand::SetEditorFont(font) => {
                self.signals.editor_font.set(font);
                self.refresh_visible_viewport();
            }

            EditorCommand::SetTerminalFont(font) => {
                self.signals.terminal_font.set(font);
            }
        }
    }

    /// Returns the number of lines in the underlying Rope buffer.
    pub fn total_lines(&self) -> usize {
        self.buffer.len_lines()
    }

    /// Returns the reactive scope owning this view model's signals.
    pub fn scope(&self) -> Scope {
        self.scope
    }

    /// Flattens hierarchical `OutlineSymbol` trees into `FlatOutlineItem` rows with depth metadata.
    pub fn flatten_symbols(symbols: &[OutlineSymbol]) -> Vec<FlatOutlineItem> {
        let mut out = Vec::new();
        for sym in symbols {
            Self::flatten_symbol_recursive(sym, 0, &mut out);
        }
        out
    }

    fn flatten_symbol_recursive(
        sym: &OutlineSymbol,
        depth: usize,
        out: &mut Vec<FlatOutlineItem>,
    ) {
        out.push(FlatOutlineItem {
            name: sym.name.clone(),
            kind: sym.kind,
            depth,
            start_line: sym.start_line,
            detail: sym.detail.clone(),
        });
        for child in &sym.children {
            Self::flatten_symbol_recursive(child, depth + 1, out);
        }
    }

    // ========================================================================
    // INTERNAL AI PREPARATION, VIRTUALIZATION & STATUS FORMATTING
    // ========================================================================

    fn prepare_ai_task_request(&mut self, task: AiTaskKind) -> Option<AiPromptRequest> {
        if let Some(cancel_tx) = self.active_ai_cancel.take() {
            let _ = cancel_tx.send(true);
        }

        if !self.signals.ai_pane_open.get_untracked() {
            self.signals.ai_pane_open.set(true);
        }

        let cursor_line = self.buffer.cursor().head.line;
        let model = self.signals.ai_model_name.get_untracked();

        match self
            .prompt_builder
            .build_from_source(&self.source_cache, cursor_line, &model, task)
        {
            Some(req) => {
                self.signals
                    .active_ai_context
                    .set(Some(req.context.clone()));
                self.signals.ai_stream_text.set(String::new());
                self.signals.ai_clean_code.set(None);
                self.signals.ai_is_streaming.set(true);
                Some(req)
            }
            None => {
                self.signals.active_ai_context.set(None);
                self.signals.ai_clean_code.set(None);
                self.signals.ai_is_streaming.set(false);
                self.signals.ai_stream_text.set(
                    "# Place cursor inside a Python function or class to slice AST context."
                        .to_string(),
                );
                None
            }
        }
    }

    fn format_buffer_in_memory(&mut self) {
        match self.formatter.format_source(&self.source_cache) {
            Ok(formatted) => {
                self.signals.last_format_error.set(None);
                if self.buffer.replace_full_preserving_cursor(&formatted) {
                    self.source_cache = formatted;
                    self.highlighter.parse_full(&self.source_cache);
                    self.signals.is_dirty.set(true);

                    let total_height = (self.buffer.len_lines() as f64) * self.line_height_px;
                    self.signals.total_content_height.set(total_height);
                    self.signals.cursor.set(self.buffer.cursor());
                    self.ensure_cursor_visible();
                    self.update_status_bar_signal();
                    self.refresh_visible_viewport();
                } else {
                    self.update_status_bar_signal();
                }
            }
            Err(FormatError::SyntaxError(msg) | FormatError::InternalError(msg)) => {
                self.signals.last_format_error.set(Some(msg));
                self.update_status_bar_signal();
            }
        }
    }

    fn load_buffer_text(&mut self, source: String) {
        self.buffer = TextBuffer::from_str(&source);
        self.source_cache = source;
        self.highlighter.parse_full(&self.source_cache);
        self.line_diagnostics.clear();
        self.signals.last_format_error.set(None);
        self.signals.is_dirty.set(false);

        let total_height = (self.buffer.len_lines() as f64) * self.line_height_px;
        self.signals.total_content_height.set(total_height);
        self.signals.scroll_offset_y.set(0.0);
        self.signals.cursor.set(self.buffer.cursor());
        self.update_status_bar_signal();
        self.refresh_visible_viewport();
    }

    fn resolve_and_read_file(&self, path: &Path) -> (PathBuf, String) {
        if classify_file_language(path) == Some(LanguageId::PydBinaryStub) {
            match self.indexer.resolve_binary_artifact(path) {
                StubResolution::StubFile(stub_path) => {
                    let text = fs::read_to_string(&stub_path).unwrap_or_default();
                    return (stub_path, text);
                }
                StubResolution::NativeSource {
                    path: native_path, ..
                } => {
                    let text = fs::read_to_string(&native_path).unwrap_or_default();
                    return (native_path, text);
                }
                StubResolution::SynthesizedStub(synthesized) => {
                    return (path.to_path_buf(), synthesized);
                }
            }
        }

        let text = fs::read_to_string(path).unwrap_or_default();
        (path.to_path_buf(), text)
    }

    fn run_ephemeral_analysis_internal(&mut self) {
        let report = self.analyzer.analyze_ephemeral(&self.source_cache);
        let flat_outline = Self::flatten_symbols(&report.symbols);

        self.rebuild_line_diagnostics(&report.diagnostics);

        self.signals.outline_items.set(flat_outline);
        self.signals.diagnostics.set(report.diagnostics);
        self.update_status_bar_signal();
        self.refresh_visible_viewport();
    }

    fn sync_after_buffer_mutation(&mut self) {
        self.source_cache = self.buffer.to_string_lossy();
        self.highlighter.parse_full(&self.source_cache);

        let total_height = (self.buffer.len_lines() as f64) * self.line_height_px;
        self.signals.total_content_height.set(total_height);
        self.signals.cursor.set(self.buffer.cursor());
        self.ensure_cursor_visible();
        self.update_status_bar_signal();
        self.refresh_visible_viewport();
    }

    fn refresh_visible_viewport(&mut self) {
        let scroll_y = self.signals.scroll_offset_y.get_untracked();
        let start_line = (scroll_y / self.line_height_px).floor() as usize;
        let visible_count = (self.viewport_height_px / self.line_height_px).ceil() as usize;
        let end_line = start_line.saturating_add(visible_count);

        let vp_slice = self.buffer.viewport_slice(start_line, end_line);
        let spans = self
            .highlighter
            .highlight_viewport(&self.source_cache, vp_slice.byte_range.clone());

        let active_line = self.buffer.cursor().head.line;
        let active_col = self.buffer.cursor().head.col;
        let mut rendered = Vec::with_capacity(vp_slice.lines.len());

        for vis_line in vp_slice.lines {
            let styled_segments = build_styled_segments(&vis_line, &spans);
            let highest_diagnostic = self.line_diagnostics.get(&vis_line.line_idx).copied();
            let is_active = vis_line.line_idx == active_line;

            rendered.push(RenderedLine {
                line_idx: vis_line.line_idx,
                gutter_number: vis_line.line_idx + 1,
                is_active_line: is_active,
                cursor_col: if is_active { Some(active_col) } else { None },
                highest_diagnostic,
                text: vis_line.text,
                styled_segments,
            });
        }

        self.signals.visible_lines.set(rendered);
    }

    fn ensure_cursor_visible(&mut self) {
        let cursor_line = self.buffer.cursor().head.line;
        let scroll_y = self.signals.scroll_offset_y.get_untracked();
        let start_line = (scroll_y / self.line_height_px).floor() as usize;
        let visible_count = (self.viewport_height_px / self.line_height_px)
            .ceil()
            .max(1.0) as usize;
        let end_line = start_line + visible_count;

        if cursor_line < start_line {
            self.signals
                .scroll_offset_y
                .set((cursor_line as f64) * self.line_height_px);
        } else if cursor_line >= end_line {
            let new_start = cursor_line.saturating_sub(visible_count - 1);
            self.signals
                .scroll_offset_y
                .set((new_start as f64) * self.line_height_px);
        }
    }

    fn rebuild_line_diagnostics(&mut self, diagnostics: &[Diagnostic]) {
        self.line_diagnostics.clear();
        let total_lines = self.buffer.len_lines();

        let trailing_empty_line = if total_lines > 1 {
            let last_slice = self.buffer.viewport_slice(total_lines - 1, total_lines);
            last_slice
                .lines
                .first()
                .map(|l| l.text.is_empty())
                .unwrap_or(false)
        } else {
            false
        };

        for diag in diagnostics {
            let mut target_line = diag.line.min(total_lines.saturating_sub(1));
            if trailing_empty_line && target_line == total_lines - 1 {
                target_line = total_lines - 2;
            }

            self.line_diagnostics
                .entry(target_line)
                .and_modify(|existing| {
                    if severity_rank(diag.severity) > severity_rank(*existing) {
                        *existing = diag.severity;
                    }
                })
                .or_insert(diag.severity);
        }
    }

    fn update_status_bar_signal(&self) {
        let head = self.buffer.cursor().head;
        let diag_count = self.line_diagnostics.len();
        let fmt_err = self.signals.last_format_error.get_untracked();
        let py_env = self.signals.python_env.get_untracked();
        let is_dirty = self.signals.is_dirty.get_untracked();
        self.signals.status_bar_text.set(Self::format_status(
            head,
            self.buffer.len_lines(),
            diag_count,
            fmt_err.as_deref(),
            py_env.as_ref(),
            is_dirty,
        ));
    }

    fn format_status(
        pos: Position,
        total_lines: usize,
        diag_count: usize,
        format_error: Option<&str>,
        python_env: Option<&PythonEnvInfo>,
        is_dirty: bool,
    ) -> String {
        let dirty_badge = if is_dirty { " [Modified]" } else { "" };
        let env_badge = match python_env {
            Some(env) => {
                let env_name = env
                    .root
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("venv");
                match env.version.as_deref() {
                    Some(ver) => format!("Python {ver} ({env_name})"),
                    None => format!("Python ({env_name})"),
                }
            }
            None => "Python (System)".to_string(),
        };

        match format_error {
            Some(err) => format!(
                "Ln {}, Col {}{} | {} lines | {} | Format aborted (SyntaxError: {})",
                pos.line + 1,
                pos.col + 1,
                dirty_badge,
                total_lines,
                env_badge,
                err
            ),
            None => format!(
                "Ln {}, Col {}{} | {} lines | {} diagnostics | {} | Ruff Ready (Ctrl+S)",
                pos.line + 1,
                pos.col + 1,
                dirty_badge,
                total_lines,
                diag_count,
                env_badge
            ),
        }
    }
}

#[inline]
fn severity_rank(sev: DiagnosticSeverity) -> u8 {
    match sev {
        DiagnosticSeverity::Error => 4,
        DiagnosticSeverity::Warning => 3,
        DiagnosticSeverity::Information => 2,
        DiagnosticSeverity::Hint => 1,
    }
}

/// Slices viewport-level Tree-sitter `HighlightSpan`s into contiguous line-local `StyledTextSegment`s.
fn build_styled_segments(
    vis_line: &VisibleLine,
    viewport_spans: &[HighlightSpan],
) -> Vec<StyledTextSegment> {
    let text_len = vis_line.text.len();
    if text_len == 0 {
        return Vec::new();
    }

    let line_start_byte = vis_line.byte_range.start;
    let line_end_byte = line_start_byte + text_len;

    let mut segments = Vec::with_capacity(8);
    let mut cursor_rel = 0usize;

    for span in viewport_spans {
        if span.byte_range.end <= line_start_byte {
            continue;
        }
        if span.byte_range.start >= line_end_byte {
            break;
        }

        let rel_start = span
            .byte_range
            .start
            .saturating_sub(line_start_byte)
            .min(text_len)
            .max(cursor_rel);
        let rel_end = span
            .byte_range
            .end
            .saturating_sub(line_start_byte)
            .min(text_len);

        if rel_start >= rel_end
            || !vis_line.text.is_char_boundary(rel_start)
            || !vis_line.text.is_char_boundary(rel_end)
        {
            continue;
        }

        if rel_start > cursor_rel {
            segments.push(StyledTextSegment {
                rel_byte_range: cursor_rel..rel_start,
                token: None,
            });
        }

        segments.push(StyledTextSegment {
            rel_byte_range: rel_start..rel_end,
            token: Some(span.token),
        });

        cursor_rel = rel_end;
    }

    if cursor_rel < text_len {
        segments.push(StyledTextSegment {
            rel_byte_range: cursor_rel..text_len,
            token: None,
        });
    }

    segments
}
