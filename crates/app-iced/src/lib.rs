//! Lide IDE - Iced Presentation Shell
//!
//! Replaces Floem GUI with The Elm Architecture (TEA) in Iced 0.13.
//! Provides zero-network in-memory TrueType font embedding, virtualized code
//! viewport rendering, and native Tokio streaming tasks for Pytest, Ollama, and Ripgrep.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

use ai_engine::{
    AiEngine, AiPromptRequest, AiStreamEvent, AiStreamHandle, AiTaskKind, AstPromptBuilder,
    LlmStreamingClient,
};
use editor_core::{MoveDirection, Position, TextBuffer, VisibleLine};
use iced::{Element, Task};
use lang_contract::{
    AstAnalyzer, CodeFormatter, Diagnostic, DiagnosticSeverity, EnclosingContext, FormatError,
    HighlightSpan, HighlightToken, LanguageId, OutlineSymbol, StubResolution, SymbolKind,
    SyntaxHighlighter,
};
use lang_python::{PythonAstAnalyzer, PythonFormatter, PythonSyntaxHighlighter};
use tokio::sync::watch;
use workspace_engine::{
    build_pytest_command_spec, classify_file_language, PythonEnvInfo, PytestCommandSpec,
    PytestExecutor, PytestFailure, PytestRunHandle, PytestRunner, PytestStreamEvent, SearchError,
    SearchMatch, VirtualenvDetector, WorkspaceFile, WorkspaceIndexer, WorkspaceSearcher,
    WorkspaceTreeNode,
};

pub mod fonts;
pub mod icons;
pub mod settings;
pub mod subscriptions;
pub mod theme;
pub mod views;

pub use settings::LideSettings;

/// Active view mode for the left sidebar panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SidebarTab {
    #[default]
    ProjectFiles,
    Structure,
}

/// Visual lifecycle state of a file in the workspace tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileItemState {
    #[default]
    Normal,
    /// Selected & opened in editor, or has unsaved edits in buffer (Orange).
    Opened,
    /// Successfully saved to disk via Ctrl+S or SaveActiveFile (Green).
    Saved,
}

/// Floating context menu state for a tree item.
#[derive(Debug, Clone, PartialEq)]
pub struct FileContextMenuState {
    pub target_path: PathBuf,
    pub is_dir: bool,
    pub position_x: f32,
    pub position_y: f32,
}

/// Inline prompt state for creating a new file or directory under a parent folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewItemPromptState {
    pub parent_dir: PathBuf,
    pub is_folder: bool,
    pub input_value: String,
}

/// Interactive prompt state for adding or opening a workspace folder in the file manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenFolderPromptState {
    pub input_value: String,
    pub error_message: Option<String>,
}

/// Active category tab inside the 2-pane Settings modal dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SettingsTab {
    #[default]
    Fonts,
    Ai,
    Theme,
    Configs,
    AiModels,
}

/// Active color scheme mode (Dark or Light).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

/// Active floating dropdown menu in the top toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ActiveMenu {
    #[default]
    None,
    File,
    Settings,
}

/// A line-local colored text span ready for Iced text layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledTextSegment {
    /// Byte range relative to `RenderedLine::text` (`0..line.text.len()`).
    pub rel_byte_range: Range<usize>,
    pub token: Option<HighlightToken>,
}

/// Self-contained view model for a single visible row inside the code editor.
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

/// Flattened representation of an `OutlineSymbol` for the Structure sidebar.
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

/// Unidirectional messages and commands dispatched by keyboard shortcuts, mouse clicks,
/// or background worker tasks in Iced TEA.
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
    /// Toggles the left sidebar between Structure and Project Files.
    ToggleStructureTab,
    /// Starts dragging the sidebar resize splitter handle.
    StartResizeSidebar,
    /// Resizes the sidebar to the given pixel width.
    ResizeSidebar(f32),
    /// Ends dragging the sidebar resize splitter handle.
    EndResizeSidebar,
    /// No-operation command for unhandled global/subscription events.
    NoOp,
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
    /// Triggered when the user clicks a specific line in the code editor.
    ClickEditorLine {
        line_idx: usize,
    },
    /// Triggered when the user clicks in empty space below the code in the editor.
    ClickEditorEmptySpace,
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
    /// Feeds a streaming token, completion, or error event from `ai-engine` into state.
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
    /// Opens the matched file, moves cursor to `(line - 1, col)`, and scrolls to match.
    ClickSearchResult(SearchMatch),
    /// Toggles the bottom Pytest Runner panel open or closed (Ctrl+Shift+T).
    TogglePytestPanel,
    /// Prepares a non-blocking `pytest` run (either for `target_file` or the full workspace).
    TriggerPytestRun {
        target_file: Option<PathBuf>,
    },
    /// Feeds a streaming line, final `PytestRunReport`, or error into state.
    ReceivePytestEvent(PytestStreamEvent),
    /// Aborts an in-flight `pytest` child process.
    CancelPytestRun,
    /// Opens failing test file, moves cursor to `(line_number - 1, 0)`, and scrolls to failure.
    ClickPytestFailure(PytestFailure),
    /// Inserts a newline with Python smart auto-indentation (`Enter`).
    InsertNewlineAutoIndent,
    /// Indents the active line by 4 PEP 8 spaces (`Tab`).
    IndentLine,
    /// Unindents the active line by up to 4 leading spaces (`Shift+Tab`).
    DedentLine,
    /// Formats active buffer with Ruff, writes atomically to `active_file_path`, clears dirty flag.
    SaveActiveFile,
    /// Toggles the specified top toolbar dropdown menu (`File` or `Settings`).
    ToggleMenu(ActiveMenu),
    /// Closes any open top toolbar dropdown menu.
    CloseMenu,
    /// Clears the active buffer for a new blank document.
    NewDocument,
    /// Opens or closes the Font Settings modal dialog.
    ToggleFontDialog,
    /// Toggles the 2-pane Settings modal dialog open or closed.
    ToggleSettingsDialog,
    /// Opens the 2-pane Settings modal dialog with a specific category tab.
    OpenSettingsDialog(SettingsTab),
    /// Closes the 2-pane Settings modal dialog.
    CloseSettingsDialog,
    /// Switches the active category tab in the Settings dialog left pane.
    SelectSettingsTab(SettingsTab),
    /// Sets the editor font size in pixels.
    SetFontSize(f32),
    /// Sets the IDE UI label font size in pixels.
    SetIdeFontSize(f32),
    /// Sets the editor line height in pixels.
    SetLineHeight(f64),
    /// Sets the local Ollama AI API endpoint.
    SetAiEndpoint(String),
    /// Sets the active local LLM model name.
    SetAiModel(String),
    /// Initiates an asynchronous probe to verify connectivity to the configured Ollama server.
    ProbeAiConnection,
    /// Receives the result of an Ollama connectivity probe (`is_connected`, optional message/error).
    AiConnectionResult {
        is_connected: bool,
        error: Option<String>,
    },
    /// Sets the active editor color theme.
    SetTheme(String),
    /// Sets the active theme mode (Dark or Light).
    SetThemeMode(ThemeMode),
    /// Toggles between Dark Mode and Light Mode.
    ToggleThemeMode,
    /// Toggles format-on-save behavior.
    ToggleFormatOnSave,
    /// Toggles line numbers visibility.
    ToggleLineNumbers,
    /// Sets the UI / IDE font family.
    SetIdeFont(String),
    /// Sets the Code Editor font family.
    SetEditorFont(String),
    /// Sets the Terminal font family.
    SetTerminalFont(String),
    /// Toggles the left-side File Manager tree visibility.
    ToggleFileManager,
    /// Expands or collapses a directory in the workspace tree.
    ToggleDirectoryExpand(PathBuf),
    /// Selects and opens a file from the file manager into the editor (sets status to Opened/Orange).
    SelectFileManagerItem(PathBuf),
    /// Opens the floating context menu for a file or folder at the specified screen position.
    OpenFileContextMenu {
        path: PathBuf,
        is_dir: bool,
        position_x: f32,
        position_y: f32,
    },
    /// Closes the floating file context menu.
    CloseFileContextMenu,
    /// Activates the inline prompt to create a new file under `parent_dir`.
    TriggerCreateFilePrompt(PathBuf),
    /// Activates the inline prompt to create a new folder under `parent_dir`.
    TriggerCreateFolderPrompt(PathBuf),
    /// Updates the text value of the inline create item prompt.
    UpdateNewItemName(String),
    /// Confirms creation of the new file/folder, updates disk & tree, and opens file if created.
    ConfirmCreateItem,
    /// Cancels the inline create item prompt.
    CancelCreateItem,
    /// Deletes the specified file or folder from disk and updates tree.
    DeleteWorkspaceEntry(PathBuf),
    /// Reveals the specified file or folder in the OS file explorer.
    RevealInExplorer(PathBuf),
    /// Re-indexes the entire active workspace directory from disk and refreshes the tree.
    RefreshWorkspaceTree,
    /// Activates the interactive prompt in the File Manager panel to open or add a workspace folder.
    TriggerOpenFolderPrompt,
    /// Updates the path text in the open/add folder prompt.
    UpdateOpenFolderPath(String),
    /// Confirms the entered folder path: creates directory if it doesn't exist, indexes it, and opens workspace.
    ConfirmOpenFolder,
    /// Cancels the open/add folder prompt.
    CancelOpenFolderPrompt,
    /// Triggers the native OS file explorer dialog directly in a background task to select a folder.
    PromptSystemFolderPicker,
    /// Receives the selected folder path from the native OS folder picker dialog.
    ReceiveSelectedFolder(Option<PathBuf>),
    /// Dismisses active modal dialog, dropdown menu, search panel, or cancels in-flight stream.
    Escape,
}

/// Connection state to the local or remote Ollama streaming LLM server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AiConnectionStatus {
    #[default]
    Disconnected,
    Checking,
    Connected,
    Error,
}

/// The unified Elm Architecture application state for Lide IDE in Iced.
pub struct LideApp {
    // Core synchronous headless engines
    pub buffer: TextBuffer,
    pub source_cache: String,
    pub highlighter: PythonSyntaxHighlighter,
    pub analyzer: PythonAstAnalyzer,
    pub formatter: PythonFormatter,
    pub indexer: WorkspaceIndexer,
    pub searcher: WorkspaceSearcher,
    pub venv_detector: VirtualenvDetector,
    pub prompt_builder: AstPromptBuilder,

    // Task cancellation handles
    pub active_ai_cancel: Option<watch::Sender<bool>>,
    pub active_pytest_cancel: Option<watch::Sender<bool>>,

    // Diagnostics cache for gutter badges (0-based line index -> highest severity)
    pub line_diagnostics: HashMap<usize, DiagnosticSeverity>,

    // Viewport geometry & metrics
    pub line_height_px: f64,
    pub viewport_height_px: f64,
    pub scroll_offset_y: f64,
    pub total_content_height_px: f64,

    // Sliced lines visible in current scroll viewport
    pub visible_lines: Vec<RenderedLine>,
    pub status_bar_text: String,
    pub outline_items: Vec<FlatOutlineItem>,
    pub workspace_files: Vec<WorkspaceFile>,
    pub active_workspace_root: Option<PathBuf>,
    pub active_file_path: Option<PathBuf>,
    pub python_env: Option<PythonEnvInfo>,
    pub diagnostics: Vec<Diagnostic>,
    pub last_format_error: Option<String>,
    pub is_dirty: bool,

    // UI Panel visibility & tabs
    pub active_sidebar_tab: SidebarTab,
    pub active_menu: ActiveMenu,
    pub font_dialog_open: bool,
    pub settings_dialog_open: bool,
    pub active_settings_tab: SettingsTab,
    pub editor_font_size: f32,
    pub ai_endpoint: String,
    pub active_theme_name: String,
    pub theme_mode: ThemeMode,
    pub current_theme: crate::theme::ThemePalette,
    pub format_on_save_enabled: bool,
    pub line_numbers_enabled: bool,

    // Typography configuration
    pub ide_font: String,
    pub ide_font_size: f32,
    pub editor_font: String,
    pub terminal_font: String,

    // Find-in-Files search panel
    pub search_panel_open: bool,
    pub search_query: String,
    pub search_results: Vec<SearchMatch>,
    pub search_error: Option<String>,

    // Pytest runner panel
    pub pytest_panel_open: bool,
    pub pytest_is_running: bool,
    pub pytest_output_lines: Vec<String>,
    pub pytest_failures: Vec<PytestFailure>,
    pub pytest_summary: Option<String>,

    // AI Assistant panel
    pub ai_pane_open: bool,
    pub ai_is_streaming: bool,
    pub active_ai_context: Option<EnclosingContext>,
    pub ai_stream_text: String,
    pub ai_clean_code: Option<String>,
    pub ai_model_name: String,
    pub ai_status: AiConnectionStatus,
    pub ai_status_message: Option<String>,

    // File Manager & Workspace Tree
    pub file_manager_open: bool,
    pub sidebar_width_px: f32,
    pub is_resizing_sidebar: bool,
    pub workspace_tree: Option<WorkspaceTreeNode>,
    pub expanded_dirs: HashSet<PathBuf>,
    pub file_states: HashMap<PathBuf, FileItemState>,
    pub file_context_menu: Option<FileContextMenuState>,
    pub new_item_prompt: Option<NewItemPromptState>,
    pub open_folder_prompt: Option<OpenFolderPromptState>,
}

pub type EditorViewModel = LideApp;

impl LideApp {
    /// Initializes a new Lide IDE application state with given viewport geometry.
    pub fn new(line_height_px: f64, viewport_height_px: f64) -> Self {
        let line_height = line_height_px.max(1.0);
        let viewport_height = viewport_height_px.max(line_height);

        let buffer = TextBuffer::new();
        let total_content_height = (buffer.len_lines() as f64) * line_height;

        let mut app = Self {
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
            scroll_offset_y: 0.0,
            total_content_height_px: total_content_height,
            visible_lines: Vec::new(),
            status_bar_text: String::new(),
            outline_items: Vec::new(),
            workspace_files: Vec::new(),
            active_workspace_root: None,
            active_file_path: None,
            python_env: None,
            diagnostics: Vec::new(),
            last_format_error: None,
            is_dirty: false,
            active_sidebar_tab: SidebarTab::ProjectFiles,
            active_menu: ActiveMenu::None,
            font_dialog_open: false,
            settings_dialog_open: false,
            active_settings_tab: SettingsTab::Fonts,
            editor_font_size: 14.0,
            ai_endpoint: "http://localhost:11434".to_string(),
            active_theme_name: "One Dark".to_string(),
            theme_mode: ThemeMode::Dark,
            current_theme: crate::theme::ThemePalette::resolve("One Dark", ThemeMode::Dark),
            format_on_save_enabled: true,
            line_numbers_enabled: true,
            ide_font: "Quicksand".to_string(),
            ide_font_size: 20.0,
            editor_font: "JetBrains Mono".to_string(),
            terminal_font: "JetBrains Mono".to_string(),
            search_panel_open: false,
            search_query: String::new(),
            search_results: Vec::new(),
            search_error: None,
            pytest_panel_open: false,
            pytest_is_running: false,
            pytest_output_lines: Vec::new(),
            pytest_failures: Vec::new(),
            pytest_summary: None,
            ai_pane_open: false,
            ai_is_streaming: false,
            active_ai_context: None,
            ai_stream_text: String::new(),
            ai_clean_code: None,
            ai_model_name: "qwen2.5-coder:7b".to_string(),
            ai_status: AiConnectionStatus::Disconnected,
            ai_status_message: None,
            file_manager_open: false,
            sidebar_width_px: 250.0,
            is_resizing_sidebar: false,
            workspace_tree: None,
            expanded_dirs: HashSet::new(),
            file_states: HashMap::new(),
            file_context_menu: None,
            new_item_prompt: None,
            open_folder_prompt: None,
        };

        app.highlighter.parse_full("");
        app.update_status_bar();
        app.refresh_visible_viewport();
        app
    }

    /// Headless constructor with standard desktop dimensions (22px line height, 792px viewport).
    pub fn new_headless() -> Self {
        Self::new(22.0, 792.0)
    }

    /// Dispatches an `EditorCommand`, synchronously mutating application state.
    /// Returns an optional `Task<EditorCommand>` for background I/O or streaming.
    pub fn dispatch(&mut self, cmd: EditorCommand) -> Task<EditorCommand> {
        match cmd {
            EditorCommand::LoadSource(source) => {
                self.load_buffer_text(source);
                Task::none()
            }

            EditorCommand::OpenWorkspace(root_path) => {
                let files = self.indexer.index_directory(&root_path);
                let tree = self.indexer.index_tree(&root_path);
                let env_override = std::env::var_os("VIRTUAL_ENV").map(PathBuf::from);
                let detected_env = self
                    .venv_detector
                    .detect(&root_path, env_override.as_deref());

                self.active_workspace_root = Some(root_path);
                self.workspace_files = files;
                self.workspace_tree = Some(tree);
                self.file_manager_open = true;
                self.python_env = detected_env;
                self.update_status_bar();
                Task::none()
            }

            EditorCommand::OpenWorkspaceFile(path) => {
                let (resolved_path, content) = self.resolve_and_read_file(&path);
                self.active_file_path = Some(resolved_path.clone());
                self.file_states.insert(resolved_path, FileItemState::Opened);
                self.load_buffer_text(content);
                self.run_ephemeral_analysis_internal();
                Task::none()
            }

            EditorCommand::ToggleFileManager => {
                self.file_manager_open = !self.file_manager_open;
                Task::none()
            }

            EditorCommand::ToggleDirectoryExpand(path) => {
                if self.expanded_dirs.contains(&path) {
                    self.expanded_dirs.remove(&path);
                } else {
                    self.expanded_dirs.insert(path);
                }
                Task::none()
            }

            EditorCommand::SelectFileManagerItem(path) => {
                self.file_context_menu = None;
                let (resolved_path, content) = self.resolve_and_read_file(&path);
                self.active_file_path = Some(resolved_path.clone());
                self.file_states.insert(resolved_path, FileItemState::Opened);
                self.load_buffer_text(content);
                self.run_ephemeral_analysis_internal();
                Task::none()
            }

            EditorCommand::OpenFileContextMenu {
                path,
                is_dir,
                position_x,
                position_y,
            } => {
                self.file_context_menu = Some(FileContextMenuState {
                    target_path: path,
                    is_dir,
                    position_x,
                    position_y,
                });
                Task::none()
            }

            EditorCommand::CloseFileContextMenu => {
                self.file_context_menu = None;
                Task::none()
            }

            EditorCommand::TriggerCreateFilePrompt(parent_dir) => {
                self.file_context_menu = None;
                self.expanded_dirs.insert(parent_dir.clone());
                self.new_item_prompt = Some(NewItemPromptState {
                    parent_dir,
                    is_folder: false,
                    input_value: String::new(),
                });
                Task::none()
            }

            EditorCommand::TriggerCreateFolderPrompt(parent_dir) => {
                self.file_context_menu = None;
                self.expanded_dirs.insert(parent_dir.clone());
                self.new_item_prompt = Some(NewItemPromptState {
                    parent_dir,
                    is_folder: true,
                    input_value: String::new(),
                });
                Task::none()
            }

            EditorCommand::UpdateNewItemName(name) => {
                if let Some(prompt) = &mut self.new_item_prompt {
                    prompt.input_value = name;
                }
                Task::none()
            }

            EditorCommand::ConfirmCreateItem => {
                if let Some(prompt) = self.new_item_prompt.take() {
                    let trimmed = prompt.input_value.trim();
                    if trimmed.is_empty() {
                        return Task::none();
                    }
                    if prompt.is_folder {
                        if let Ok(new_dir) = workspace_engine::create_folder(&prompt.parent_dir, trimmed) {
                            self.expanded_dirs.insert(prompt.parent_dir);
                            self.expanded_dirs.insert(new_dir);
                            if let Some(root) = &self.active_workspace_root {
                                self.workspace_tree = Some(self.indexer.index_tree(root));
                                self.workspace_files = self.indexer.index_directory(root);
                            }
                        }
                    } else {
                        if let Ok(new_file) = workspace_engine::create_file(&prompt.parent_dir, trimmed) {
                            self.expanded_dirs.insert(prompt.parent_dir);
                            if let Some(root) = &self.active_workspace_root {
                                self.workspace_tree = Some(self.indexer.index_tree(root));
                                self.workspace_files = self.indexer.index_directory(root);
                            }
                            return self.update(EditorCommand::SelectFileManagerItem(new_file));
                        }
                    }
                }
                Task::none()
            }

            EditorCommand::CancelCreateItem => {
                self.new_item_prompt = None;
                Task::none()
            }

            EditorCommand::DeleteWorkspaceEntry(path) => {
                self.file_context_menu = None;
                if workspace_engine::delete_entry(&path).is_ok() {
                    let should_close_buffer = if let Some(active) = &self.active_file_path {
                        active == &path || active.starts_with(&path)
                    } else {
                        false
                    };

                    if should_close_buffer {
                        if let Some(active) = self.active_file_path.take() {
                            self.file_states.remove(&active);
                        }
                        self.load_buffer_text(String::new());
                    }

                    if let Some(root) = &self.active_workspace_root {
                        self.workspace_tree = Some(self.indexer.index_tree(root));
                        self.workspace_files = self.indexer.index_directory(root);
                    }
                }
                Task::none()
            }

            EditorCommand::RevealInExplorer(path) => {
                self.file_context_menu = None;
                let _ = workspace_engine::reveal_in_system_explorer(&path);
                Task::none()
            }

            EditorCommand::RefreshWorkspaceTree => {
                self.file_context_menu = None;
                if let Some(root) = self.active_workspace_root.clone() {
                    self.workspace_tree = Some(self.indexer.index_tree(&root));
                    self.workspace_files = self.indexer.index_directory(&root);
                }
                Task::none()
            }

            EditorCommand::TriggerOpenFolderPrompt => {
                let default_path = std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| ".".to_string());
                self.open_folder_prompt = Some(OpenFolderPromptState {
                    input_value: default_path,
                    error_message: None,
                });
                Task::none()
            }

            EditorCommand::UpdateOpenFolderPath(path) => {
                if let Some(prompt) = &mut self.open_folder_prompt {
                    prompt.input_value = path;
                    prompt.error_message = None;
                }
                Task::none()
            }

            EditorCommand::ConfirmOpenFolder => {
                if let Some(prompt) = self.open_folder_prompt.clone() {
                    let trimmed = prompt.input_value.trim();
                    if trimmed.is_empty() {
                        if let Some(p) = &mut self.open_folder_prompt {
                            p.error_message = Some("Folder path cannot be empty".to_string());
                        }
                        return Task::none();
                    }

                    let target_path = PathBuf::from(trimmed);
                    // If the folder does not exist, create it so the user can immediately start working on it
                    if !target_path.exists() {
                        if let Err(err) = std::fs::create_dir_all(&target_path) {
                            if let Some(p) = &mut self.open_folder_prompt {
                                p.error_message = Some(format!("Failed to create folder: {err}"));
                            }
                            return Task::none();
                        }
                    }

                    let final_path = target_path.canonicalize().unwrap_or(target_path);
                    self.open_folder_prompt = None;
                    return self.update(EditorCommand::OpenWorkspace(final_path));
                }
                Task::none()
            }

            EditorCommand::CancelOpenFolderPrompt => {
                self.open_folder_prompt = None;
                Task::none()
            }

            EditorCommand::PromptSystemFolderPicker => {
                Task::perform(
                    async {
                        tokio::task::spawn_blocking(workspace_engine::spawn_native_folder_picker)
                            .await
                            .ok()
                            .flatten()
                    },
                    EditorCommand::ReceiveSelectedFolder,
                )
            }

            EditorCommand::ReceiveSelectedFolder(maybe_path) => {
                if let Some(folder_path) = maybe_path {
                    self.open_folder_prompt = None;
                    return self.update(EditorCommand::OpenWorkspace(folder_path));
                }
                Task::none()
            }

            EditorCommand::SwitchSidebarTab(tab) => {
                self.active_sidebar_tab = tab;
                Task::none()
            }

            EditorCommand::ToggleStructureTab => {
                self.active_sidebar_tab = match self.active_sidebar_tab {
                    SidebarTab::Structure => SidebarTab::ProjectFiles,
                    SidebarTab::ProjectFiles => SidebarTab::Structure,
                };
                Task::none()
            }

            EditorCommand::StartResizeSidebar => {
                self.is_resizing_sidebar = true;
                Task::none()
            }

            EditorCommand::ResizeSidebar(new_width) => {
                if self.is_resizing_sidebar {
                    self.sidebar_width_px = new_width.clamp(140.0, 700.0);
                }
                Task::none()
            }

            EditorCommand::EndResizeSidebar => {
                self.is_resizing_sidebar = false;
                Task::none()
            }

            EditorCommand::NoOp => Task::none(),

            EditorCommand::ScrollToPixel {
                offset_y,
                viewport_height,
            } => {
                self.viewport_height_px = viewport_height.max(self.line_height_px);
                let max_scroll = ((self.buffer.len_lines() as f64) * self.line_height_px
                    - self.viewport_height_px)
                    .max(0.0);
                self.scroll_offset_y = offset_y.clamp(0.0, max_scroll);
                self.refresh_visible_viewport();
                Task::none()
            }

            EditorCommand::InsertText(text) => {
                if text.is_empty() || self.is_modal_or_prompt_active() {
                    return Task::none();
                }
                self.buffer.insert_at_cursor(&text);
                self.is_dirty = true;
                self.sync_after_buffer_mutation();
                Task::none()
            }

            EditorCommand::InsertNewlineAutoIndent => {
                if self.is_modal_or_prompt_active() {
                    return Task::none();
                }
                self.buffer.insert_newline_auto_indent();
                self.is_dirty = true;
                self.sync_after_buffer_mutation();
                Task::none()
            }

            EditorCommand::IndentLine => {
                if self.is_modal_or_prompt_active() {
                    return Task::none();
                }
                self.buffer.indent_line();
                self.is_dirty = true;
                self.sync_after_buffer_mutation();
                Task::none()
            }

            EditorCommand::DedentLine => {
                if self.is_modal_or_prompt_active() {
                    return Task::none();
                }
                let prev_bytes = self.buffer.len_bytes();
                self.buffer.dedent_line();
                if self.buffer.len_bytes() != prev_bytes {
                    self.is_dirty = true;
                    self.sync_after_buffer_mutation();
                }
                Task::none()
            }

            EditorCommand::Backspace => {
                if self.is_modal_or_prompt_active() {
                    return Task::none();
                }
                let prev_bytes = self.buffer.len_bytes();
                self.buffer.delete_backward();
                if self.buffer.len_bytes() != prev_bytes {
                    self.is_dirty = true;
                }
                self.sync_after_buffer_mutation();
                Task::none()
            }

            EditorCommand::MoveCursor(dir) => {
                if self.is_modal_or_prompt_active() {
                    return Task::none();
                }
                self.buffer.move_cursor(dir);
                self.ensure_cursor_visible();
                self.update_status_bar();
                self.refresh_visible_viewport();
                Task::none()
            }

            EditorCommand::SetCursor(pos) => {
                self.buffer.set_cursor(pos);
                self.ensure_cursor_visible();
                self.update_status_bar();
                self.refresh_visible_viewport();
                Task::none()
            }

            EditorCommand::ClickEditorLine { line_idx } => {
                let mouse_pos = crate::subscriptions::get_last_mouse_pos();
                let gutter_w = if self.line_numbers_enabled { 72.0 } else { 16.0 };
                let code_start_x = if self.file_manager_open {
                    self.sidebar_width_px + 4.0 + gutter_w
                } else {
                    gutter_w
                };
                let char_width = (self.editor_font_size * 0.60).max(1.0);
                let rel_x = (mouse_pos.x - code_start_x).max(0.0);
                let col = (rel_x / char_width).round() as usize;
                let line_len = self.buffer.line_grapheme_len(line_idx);
                let clamped_col = col.min(line_len);
                self.buffer.set_cursor(Position::new(line_idx, clamped_col));
                self.ensure_cursor_visible();
                self.update_status_bar();
                self.refresh_visible_viewport();
                Task::none()
            }

            EditorCommand::ClickEditorEmptySpace => {
                let total_lines = self.buffer.len_lines();
                let last_line = total_lines.saturating_sub(1);
                let last_col = self.buffer.line_grapheme_len(last_line);
                self.buffer.set_cursor(Position::new(last_line, last_col));
                self.ensure_cursor_visible();
                self.update_status_bar();
                self.refresh_visible_viewport();
                Task::none()
            }

            EditorCommand::ClickStructureSymbol { start_line } => {
                self.buffer.set_cursor(Position::new(start_line, 0));
                let target_px = (start_line as f64) * self.line_height_px;
                let max_scroll = ((self.buffer.len_lines() as f64) * self.line_height_px
                    - self.viewport_height_px)
                    .max(0.0);
                self.scroll_offset_y = target_px.clamp(0.0, max_scroll);
                self.update_status_bar();
                self.refresh_visible_viewport();
                Task::none()
            }

            EditorCommand::RunEphemeralAnalysis => {
                self.run_ephemeral_analysis_internal();
                Task::none()
            }

            EditorCommand::FormatDocument => {
                self.format_buffer_in_memory();
                Task::none()
            }

            EditorCommand::SaveActiveFile => {
                if self.format_on_save_enabled {
                    self.format_buffer_in_memory();
                }
                if let Some(target_path) = self.active_file_path.clone() {
                    let temp_path = target_path.with_extension("pyrust.tmp");
                    if fs::write(&temp_path, &self.source_cache).is_ok() {
                        let _ = fs::rename(&temp_path, &target_path)
                            .or_else(|_| fs::write(&target_path, &self.source_cache));
                    } else {
                        let _ = fs::write(&target_path, &self.source_cache);
                    }
                    self.file_states.insert(target_path, FileItemState::Saved);
                }
                self.is_dirty = false;
                self.run_ephemeral_analysis_internal();
                Task::none()
            }

            EditorCommand::Undo => {
                if self.buffer.undo() {
                    self.last_format_error = None;
                    self.sync_after_buffer_mutation();
                }
                Task::none()
            }

            EditorCommand::Redo => {
                if self.buffer.redo() {
                    self.last_format_error = None;
                    self.sync_after_buffer_mutation();
                }
                Task::none()
            }

            EditorCommand::ToggleAiPane => {
                self.ai_pane_open = !self.ai_pane_open;
                Task::none()
            }

            EditorCommand::TriggerAiTask(task) => {
                if let Some(req) = self.prepare_ai_task_request(task) {
                    let (host, port) = ai_engine::parse_ollama_endpoint(&self.ai_endpoint);
                    let client = ai_engine::OllamaStreamingClient::new(&host, port);
                    let ai_engine = AiEngine::new(client);
                    let handle = ai_engine.start_stream(req);
                    self.active_ai_cancel = Some(handle.cancel_sender());
                    let mut rx = handle.rx;
                    return Task::stream(iced::stream::channel(128, move |mut output| async move {
                        use iced::futures::SinkExt;
                        while let Some(event) = rx.recv().await {
                            if output
                                .send(EditorCommand::ReceiveAiStreamEvent(event))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                    }));
                }
                Task::none()
            }

            EditorCommand::ReceiveAiStreamEvent(event) => {
                match event {
                    AiStreamEvent::Token(chunk) => {
                        self.ai_stream_text.push_str(&chunk);
                    }
                    AiStreamEvent::Completed {
                        raw_response,
                        clean_python_code,
                    } => {
                        self.ai_stream_text = raw_response;
                        self.ai_clean_code = Some(clean_python_code);
                        self.ai_is_streaming = false;
                    }
                    AiStreamEvent::Cancelled => {
                        self.ai_is_streaming = false;
                    }
                    AiStreamEvent::Error(err) => {
                        self.ai_stream_text = format!("# AI Error:\n# {err}");
                        self.ai_is_streaming = false;
                    }
                }
                Task::none()
            }

            EditorCommand::CancelAiStream => {
                if let Some(cancel_tx) = self.active_ai_cancel.take() {
                    let _ = cancel_tx.send(true);
                }
                self.ai_is_streaming = false;
                Task::none()
            }

            EditorCommand::ApplyAiSuggestion => {
                let maybe_ctx = self.active_ai_context.clone();
                let maybe_code = self.ai_clean_code.clone();

                if let (Some(ctx), Some(clean_code)) = (maybe_ctx, maybe_code) {
                    if self
                        .buffer
                        .replace_line_range(ctx.start_line, ctx.end_line, &clean_code)
                    {
                        self.is_dirty = true;
                        self.sync_after_buffer_mutation();
                        self.run_ephemeral_analysis_internal();
                    }
                }
                Task::none()
            }

            EditorCommand::ToggleSearchPanel => {
                self.search_panel_open = !self.search_panel_open;
                Task::none()
            }

            EditorCommand::SetSearchQuery(query) => {
                self.search_query = query;
                Task::none()
            }

            EditorCommand::ExecuteWorkspaceSearch { max_results } => {
                let query = self.search_query.clone();
                if query.trim().is_empty() {
                    self.search_results.clear();
                    self.search_error = None;
                    return Task::none();
                }

                let Some(root) = self.active_workspace_root.clone() else {
                    self.search_results.clear();
                    self.search_error = Some("No workspace root open".to_string());
                    return Task::none();
                };

                match self.searcher.search_regex(&root, &query, max_results) {
                    Ok(matches) => {
                        self.search_error = None;
                        self.search_results = matches;
                    }
                    Err(SearchError::InvalidRegex(err) | SearchError::IoError(err)) => {
                        self.search_results.clear();
                        self.search_error = Some(err);
                    }
                }
                Task::none()
            }

            EditorCommand::ClickSearchResult(hit) => {
                let needs_file_load = self.active_file_path.as_ref() != Some(&hit.path);
                if needs_file_load {
                    let (resolved_path, content) = self.resolve_and_read_file(&hit.path);
                    self.active_file_path = Some(resolved_path);
                    self.load_buffer_text(content);
                    self.run_ephemeral_analysis_internal();
                }

                let target_line = hit.line_number.saturating_sub(1);
                let target_pos = Position::new(target_line, hit.column);
                self.buffer.set_cursor(target_pos);
                self.ensure_cursor_visible();
                self.update_status_bar();
                self.refresh_visible_viewport();
                Task::none()
            }

            EditorCommand::TogglePytestPanel => {
                self.pytest_panel_open = !self.pytest_panel_open;
                Task::none()
            }

            EditorCommand::TriggerPytestRun { target_file } => {
                if let Some(spec) = self.prepare_pytest_spec(target_file.as_deref()) {
                    let runner = PytestRunner::new(workspace_engine::ProcessPytestExecutor);
                    let handle = runner.start_run(spec);
                    self.active_pytest_cancel = Some(handle.cancel_sender());
                    let mut rx = handle.rx;
                    return Task::stream(iced::stream::channel(128, move |mut output| async move {
                        use iced::futures::SinkExt;
                        while let Some(event) = rx.recv().await {
                            if output
                                .send(EditorCommand::ReceivePytestEvent(event))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                    }));
                }
                Task::none()
            }

            EditorCommand::ReceivePytestEvent(event) => {
                match event {
                    PytestStreamEvent::OutputLine(line) => {
                        self.pytest_output_lines.push(line);
                    }
                    PytestStreamEvent::Completed(report) => {
                        let summary = format!("{} failed, {} passed", report.failed, report.passed);
                        self.pytest_summary = Some(summary);
                        self.pytest_failures = report.failures;
                        self.pytest_is_running = false;
                    }
                    PytestStreamEvent::Cancelled => {
                        self.pytest_summary = Some("Pytest cancelled".to_string());
                        self.pytest_is_running = false;
                    }
                    PytestStreamEvent::Error(err) => {
                        self.pytest_summary = Some(format!("Pytest error: {err}"));
                        self.pytest_is_running = false;
                    }
                }
                Task::none()
            }

            EditorCommand::CancelPytestRun => {
                if let Some(cancel_tx) = self.active_pytest_cancel.take() {
                    let _ = cancel_tx.send(true);
                }
                self.pytest_is_running = false;
                Task::none()
            }

            EditorCommand::ClickPytestFailure(failure) => {
                let needs_load = self.active_file_path.as_ref() != Some(&failure.path);
                if needs_load {
                    let (resolved_path, content) = self.resolve_and_read_file(&failure.path);
                    self.active_file_path = Some(resolved_path);
                    self.load_buffer_text(content);
                    self.run_ephemeral_analysis_internal();
                }

                let target_line = failure.line_number.saturating_sub(1);
                self.buffer.set_cursor(Position::new(target_line, 0));
                self.ensure_cursor_visible();
                self.update_status_bar();
                self.refresh_visible_viewport();
                Task::none()
            }

            EditorCommand::ToggleMenu(menu) => {
                if self.active_menu == menu {
                    self.active_menu = ActiveMenu::None;
                } else {
                    self.active_menu = menu;
                }
                Task::none()
            }

            EditorCommand::CloseMenu => {
                self.active_menu = ActiveMenu::None;
                Task::none()
            }

            EditorCommand::NewDocument => {
                self.load_buffer_text(String::new());
                self.active_file_path = None;
                self.is_dirty = false;
                self.active_menu = ActiveMenu::None;
                self.run_ephemeral_analysis_internal();
                Task::none()
            }

            EditorCommand::ToggleFontDialog => {
                self.settings_dialog_open = !self.settings_dialog_open;
                self.active_settings_tab = SettingsTab::Fonts;
                self.font_dialog_open = self.settings_dialog_open;
                self.active_menu = ActiveMenu::None;
                Task::none()
            }

            EditorCommand::ToggleSettingsDialog => {
                self.settings_dialog_open = !self.settings_dialog_open;
                self.font_dialog_open = self.settings_dialog_open;
                self.active_menu = ActiveMenu::None;
                Task::none()
            }

            EditorCommand::OpenSettingsDialog(tab) => {
                self.settings_dialog_open = true;
                self.font_dialog_open = true;
                self.active_settings_tab = tab;
                self.active_menu = ActiveMenu::None;
                Task::none()
            }

            EditorCommand::CloseSettingsDialog => {
                self.settings_dialog_open = false;
                self.font_dialog_open = false;
                Task::none()
            }

            EditorCommand::SelectSettingsTab(tab) => {
                self.active_settings_tab = tab;
                Task::none()
            }

            EditorCommand::SetFontSize(sz) => {
                self.editor_font_size = sz.clamp(8.0, 32.0);
                let min_lh = (self.editor_font_size * 1.4).round() as f64;
                if self.line_height_px < min_lh {
                    self.line_height_px = min_lh;
                }
                self.refresh_visible_viewport();
                let _ = self.persist_settings();
                Task::none()
            }

            EditorCommand::SetLineHeight(lh) => {
                let min_lh = (self.editor_font_size as f64).max(14.0);
                self.line_height_px = lh.clamp(min_lh, 48.0);
                self.refresh_visible_viewport();
                let _ = self.persist_settings();
                Task::none()
            }

            EditorCommand::SetAiEndpoint(ep) => {
                self.ai_endpoint = ep;
                self.ai_status = AiConnectionStatus::Disconnected;
                self.ai_status_message = None;
                let _ = self.persist_settings();
                Task::none()
            }

            EditorCommand::SetAiModel(model) => {
                self.ai_model_name = model;
                self.update_status_bar();
                let _ = self.persist_settings();
                Task::none()
            }

            EditorCommand::ProbeAiConnection => {
                self.ai_status = AiConnectionStatus::Checking;
                self.ai_status_message = Some("Probing Ollama endpoint...".to_string());
                let endpoint = self.ai_endpoint.clone();
                Task::future(async move {
                    let (host, port) = ai_engine::parse_ollama_endpoint(&endpoint);
                    let client = ai_engine::OllamaStreamingClient::new(&host, port);
                    match client.check_health().await {
                        Ok(models) => {
                            let msg = if models.is_empty() {
                                "Connected (no models loaded)".to_string()
                            } else {
                                format!(
                                    "Connected ({} model{} available)",
                                    models.len(),
                                    if models.len() == 1 { "" } else { "s" }
                                )
                            };
                            EditorCommand::AiConnectionResult {
                                is_connected: true,
                                error: Some(msg),
                            }
                        }
                        Err(err) => EditorCommand::AiConnectionResult {
                            is_connected: false,
                            error: Some(err),
                        },
                    }
                })
            }

            EditorCommand::AiConnectionResult { is_connected, error } => {
                if is_connected {
                    self.ai_status = AiConnectionStatus::Connected;
                    self.ai_status_message = error;
                } else {
                    self.ai_status = AiConnectionStatus::Error;
                    self.ai_status_message = error;
                }
                Task::none()
            }

            EditorCommand::SetTheme(theme) => {
                self.active_theme_name = theme;
                self.current_theme = crate::theme::ThemePalette::resolve(&self.active_theme_name, self.theme_mode);
                let _ = self.persist_settings();
                Task::none()
            }

            EditorCommand::SetThemeMode(mode) => {
                self.theme_mode = mode;
                match mode {
                    ThemeMode::Dark => {
                        if !self.active_theme_name.contains("Dark")
                            && !self.active_theme_name.contains("Nord")
                            && !self.active_theme_name.contains("Dracula")
                            && !self.active_theme_name.contains("Lide Original")
                            && !self.active_theme_name.contains("Cerebral")
                        {
                            self.active_theme_name = "One Dark".to_string();
                        }
                    }
                    ThemeMode::Light => {
                        if !self.active_theme_name.contains("Light")
                            && !self.active_theme_name.contains("Lide Original")
                            && !self.active_theme_name.contains("Cerebral")
                        {
                            self.active_theme_name = "One Light".to_string();
                        }
                    }
                }
                self.current_theme = crate::theme::ThemePalette::resolve(&self.active_theme_name, self.theme_mode);
                let _ = self.persist_settings();
                Task::none()
            }

            EditorCommand::ToggleThemeMode => {
                let new_mode = match self.theme_mode {
                    ThemeMode::Dark => ThemeMode::Light,
                    ThemeMode::Light => ThemeMode::Dark,
                };
                self.dispatch(EditorCommand::SetThemeMode(new_mode))
            }

            EditorCommand::ToggleFormatOnSave => {
                self.format_on_save_enabled = !self.format_on_save_enabled;
                let _ = self.persist_settings();
                Task::none()
            }

            EditorCommand::ToggleLineNumbers => {
                self.line_numbers_enabled = !self.line_numbers_enabled;
                self.refresh_visible_viewport();
                let _ = self.persist_settings();
                Task::none()
            }

            EditorCommand::SetIdeFont(font) => {
                self.ide_font = font;
                let _ = self.persist_settings();
                Task::none()
            }

            EditorCommand::SetIdeFontSize(sz) => {
                self.ide_font_size = sz.clamp(10.0, 36.0);
                let _ = self.persist_settings();
                Task::none()
            }

            EditorCommand::SetEditorFont(font) => {
                self.editor_font = font;
                self.refresh_visible_viewport();
                let _ = self.persist_settings();
                Task::none()
            }

            EditorCommand::SetTerminalFont(font) => {
                self.terminal_font = font;
                let _ = self.persist_settings();
                Task::none()
            }

            EditorCommand::Escape => {
                if self.file_context_menu.is_some() {
                    self.file_context_menu = None;
                } else if self.new_item_prompt.is_some() {
                    self.new_item_prompt = None;
                } else if self.open_folder_prompt.is_some() {
                    self.open_folder_prompt = None;
                } else if self.settings_dialog_open || self.font_dialog_open {
                    self.settings_dialog_open = false;
                    self.font_dialog_open = false;
                } else if self.active_menu != ActiveMenu::None {
                    self.active_menu = ActiveMenu::None;
                } else if self.search_panel_open {
                    self.search_panel_open = false;
                } else if self.ai_is_streaming {
                    return self.dispatch(EditorCommand::CancelAiStream);
                } else if self.pytest_is_running {
                    return self.dispatch(EditorCommand::CancelPytestRun);
                }
                Task::none()
            }
        }
    }

    /// Primary TEA update function.
    pub fn update(&mut self, message: EditorCommand) -> Task<EditorCommand> {
        self.dispatch(message)
    }

    /// Renders the complete application layout using Iced widgets.
    pub fn view(&self) -> Element<'_, EditorCommand> {
        views::render_app(self)
    }

    /// Global event subscription for keyboard shortcuts and background event streams.
    pub fn subscription(&self) -> iced::Subscription<EditorCommand> {
        subscriptions::app_subscriptions(self)
    }

    /// Number of lines in the underlying Rope buffer.
    pub fn total_lines(&self) -> usize {
        self.buffer.len_lines()
    }

    /// Returns true if an interactive modal, prompt, search input, or context menu is currently open.
    pub fn is_modal_or_prompt_active(&self) -> bool {
        self.file_context_menu.is_some()
            || self.new_item_prompt.is_some()
            || self.open_folder_prompt.is_some()
            || self.settings_dialog_open
            || self.font_dialog_open
            || self.active_menu != ActiveMenu::None
            || self.search_panel_open
    }

    /// Synchronizes runtime UI state from a loaded `LideSettings` struct.
    pub fn apply_settings(&mut self, settings: &LideSettings) {
        self.active_theme_name = settings.theme_name.clone();
        self.theme_mode = match settings.theme_mode.as_str() {
            "Light" => ThemeMode::Light,
            _ => ThemeMode::Dark,
        };
        self.current_theme = crate::theme::ThemePalette::resolve(&self.active_theme_name, self.theme_mode);
        self.editor_font_size = settings.editor_font_size.clamp(8.0, 32.0);
        self.ide_font_size = if settings.ide_font_size > 0.0 {
            settings.ide_font_size.clamp(10.0, 36.0)
        } else {
            20.0
        };
        let min_lh = (self.editor_font_size * 1.4).round() as f64;
        self.line_height_px = settings.line_height_px.max(min_lh).clamp(14.0, 48.0);
        self.format_on_save_enabled = settings.format_on_save_enabled;
        self.line_numbers_enabled = settings.line_numbers_enabled;
        self.ai_endpoint = settings.ai_endpoint.clone();
        self.ai_model_name = settings.ai_model_name.clone();
        self.editor_font = settings.editor_font.clone();
        self.refresh_visible_viewport();
        self.update_status_bar();
    }

    /// Active UI label font size for standard interface elements (default: 20.0 px).
    pub fn ui_font_size(&self) -> f32 {
        self.ide_font_size
    }

    /// Scaled font size for secondary / caption text (0.8 * ide_font_size).
    pub fn ui_caption_size(&self) -> f32 {
        (self.ide_font_size * 0.8).round().max(10.0)
    }

    /// Scaled font size for headings and titles (1.2 * ide_font_size).
    pub fn ui_title_size(&self) -> f32 {
        (self.ide_font_size * 1.2).round().max(14.0)
    }

    /// Persists current settings to `~/.lide/settings.json`.
    pub fn persist_settings(&self) -> Result<(), String> {
        let settings = LideSettings::from_app(self);
        settings.save()
    }

    /// Starts a Pytest run on the provided `PytestRunner<E>`.
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

        self.pytest_panel_open = true;
        self.pytest_output_lines.clear();
        self.pytest_failures.clear();
        self.pytest_summary = Some("Running pytest...".to_string());
        self.pytest_is_running = true;

        let root = self
            .active_workspace_root
            .clone()
            .unwrap_or_else(|| PathBuf::from("."));
        let py_env = self.python_env.clone();

        Some(build_pytest_command_spec(
            &root,
            py_env.as_ref(),
            target_file,
        ))
    }

    /// Starts an AI completion stream on the provided `AiEngine<C>`.
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

    fn prepare_ai_task_request(&mut self, task: AiTaskKind) -> Option<AiPromptRequest> {
        if let Some(cancel_tx) = self.active_ai_cancel.take() {
            let _ = cancel_tx.send(true);
        }

        self.ai_pane_open = true;
        let cursor_line = self.buffer.cursor().head.line;
        let model = self.ai_model_name.clone();

        match self
            .prompt_builder
            .build_from_source(&self.source_cache, cursor_line, &model, task)
        {
            Some(req) => {
                self.active_ai_context = Some(req.context.clone());
                self.ai_stream_text.clear();
                self.ai_clean_code = None;
                self.ai_is_streaming = true;
                Some(req)
            }
            None => {
                self.active_ai_context = None;
                self.ai_clean_code = None;
                self.ai_is_streaming = false;
                self.ai_stream_text =
                    "# Place cursor inside a Python function or class to slice AST context."
                        .to_string();
                None
            }
        }
    }

    fn format_buffer_in_memory(&mut self) {
        match self.formatter.format_source(&self.source_cache) {
            Ok(formatted) => {
                self.last_format_error = None;
                if self.buffer.replace_full_preserving_cursor(&formatted) {
                    self.source_cache = formatted;
                    self.highlighter.parse_full(&self.source_cache);
                    self.is_dirty = true;
                    self.total_content_height_px =
                        (self.buffer.len_lines() as f64) * self.line_height_px;
                    self.ensure_cursor_visible();
                    self.update_status_bar();
                    self.refresh_visible_viewport();
                } else {
                    self.update_status_bar();
                }
            }
            Err(FormatError::SyntaxError(msg) | FormatError::InternalError(msg)) => {
                self.last_format_error = Some(msg);
                self.update_status_bar();
            }
        }
    }

    fn load_buffer_text(&mut self, source: String) {
        self.buffer = TextBuffer::from_str(&source);
        self.source_cache = source;
        self.highlighter.parse_full(&self.source_cache);
        self.line_diagnostics.clear();
        self.last_format_error = None;
        self.is_dirty = false;
        self.total_content_height_px = (self.buffer.len_lines() as f64) * self.line_height_px;
        self.scroll_offset_y = 0.0;
        self.update_status_bar();
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
        self.outline_items = flat_outline;
        self.diagnostics = report.diagnostics;
        self.update_status_bar();
        self.refresh_visible_viewport();
    }

    fn sync_after_buffer_mutation(&mut self) {
        if let Some(path) = self.active_file_path.clone() {
            self.file_states.insert(path, FileItemState::Opened);
        }
        self.source_cache = self.buffer.to_string_lossy();
        self.highlighter.parse_full(&self.source_cache);
        self.total_content_height_px = (self.buffer.len_lines() as f64) * self.line_height_px;
        self.ensure_cursor_visible();
        self.update_status_bar();
        self.refresh_visible_viewport();
    }

    pub fn refresh_visible_viewport(&mut self) {
        let scroll_y = self.scroll_offset_y;
        let line_h = self.line_height_px.max(1.0);
        let raw_start = (scroll_y / line_h).floor() as usize;
        let visible_count = (self.viewport_height_px / line_h).ceil().max(25.0) as usize;
        let overscan = 10;
        let start_line = raw_start.saturating_sub(overscan);
        let end_line = raw_start.saturating_add(visible_count).saturating_add(overscan);

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

        self.visible_lines = rendered;
    }

    fn ensure_cursor_visible(&mut self) {
        let cursor_line = self.buffer.cursor().head.line;
        let scroll_y = self.scroll_offset_y;
        let start_line = (scroll_y / self.line_height_px).floor() as usize;
        let visible_count = (self.viewport_height_px / self.line_height_px)
            .ceil()
            .max(1.0) as usize;
        let end_line = start_line + visible_count;

        if cursor_line < start_line {
            self.scroll_offset_y = (cursor_line as f64) * self.line_height_px;
        } else if cursor_line >= end_line {
            let new_start = cursor_line.saturating_sub(visible_count - 1);
            self.scroll_offset_y = (new_start as f64) * self.line_height_px;
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

    fn update_status_bar(&mut self) {
        let head = self.buffer.cursor().head;
        let diag_count = self.line_diagnostics.len();
        let fmt_err = self.last_format_error.as_deref();
        let py_env = self.python_env.as_ref();
        let is_dirty = self.is_dirty;

        self.status_bar_text = Self::format_status(
            head,
            self.buffer.len_lines(),
            diag_count,
            fmt_err,
            py_env,
            is_dirty,
        );
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

    fn flatten_symbols(symbols: &[OutlineSymbol]) -> Vec<FlatOutlineItem> {
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
