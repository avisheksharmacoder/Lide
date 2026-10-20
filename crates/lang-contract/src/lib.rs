use std::ops::Range;
use std::path::{Path, PathBuf};
use editor_core::Position;

/// Supported languages across current and future milestones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageId {
    Python,
    Cython,
    C,
    Cpp,
    Rust,
    PydBinaryStub,
}

/// Theme-agnostic semantic highlight categories mapped to colors by Floem in Milestone 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightToken {
    Keyword,
    Function,
    Method,
    Class,
    Type,
    Decorator,
    String,
    Number,
    Boolean,
    Comment,
    Operator,
    Builtin,
    Parameter,
}

/// A highlighted byte span strictly within the requested viewport range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub byte_range: Range<usize>,
    pub token: HighlightToken,
}

/// Describes a buffer edit so incremental parsers (Tree-sitter) can update in <500µs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputEditSummary {
    pub start_byte: usize,
    pub old_end_byte: usize,
    pub new_end_byte: usize,
    pub start_pos: Position,
    pub old_end_pos: Position,
    pub new_end_pos: Position,
}

/// Symbol categories for the PyCharm-style Structure sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Module,
    Class,
    Method,
    Function,
    AsyncFunction,
    Variable,
    Constant,
    /// Reserved for Phase 2 Cython (`cdef` / `cpdef`) support.
    CdefFunction,
    /// Reserved for Phase 3 C/C++/Rust support.
    Struct,
}

/// Lightweight, owned AST outline node. Survives after the heavy parser AST is dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineSymbol {
    pub name: String,
    pub kind: SymbolKind,
    /// Zero-based start line.
    pub start_line: usize,
    /// Zero-based end line (inclusive).
    pub end_line: usize,
    /// Optional signature or return type annotation (e.g. `"-> list[float]"`).
    pub detail: Option<String>,
    /// Decorator names attached to this symbol (e.g. `["dataclass"]` or `["staticmethod"]`).
    pub decorators: Vec<String>,
    /// Nested symbols (e.g. methods inside a Python `class`).
    pub children: Vec<OutlineSymbol>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

/// Compact diagnostic emitted by syntax parsers or linters (Ruff).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub byte_range: Range<usize>,
    pub line: usize,
    pub col: usize,
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
}

/// Combined output of an ephemeral background AST pass.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnalysisReport {
    pub symbols: Vec<OutlineSymbol>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Surgical AST slice sent to `ollama-rs` so local LLMs only process relevant code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnclosingContext {
    pub symbol_name: String,
    pub kind: SymbolKind,
    pub start_line: usize,
    pub end_line: usize,
    /// Top-level `import` and `from ... import ...` lines in the module.
    pub module_imports: Vec<String>,
    /// Exact source text of the enclosing function or class.
    pub enclosing_source: String,
}

/// Resolution result when a user opens or jumps into a compiled `.pyd` / `.so` extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StubResolution {
    /// Found an adjacent `.pyi` typing stub file.
    StubFile(PathBuf),
    /// Found the underlying Cython (`.pyx`), C++, or Rust source file in the workspace.
    NativeSource {
        path: PathBuf,
        language: LanguageId,
    },
    /// No source/stub on disk; provides a synthesized read-only `.pyi` buffer description.
    SynthesizedStub(String),
}

// ============================================================================
// LANGUAGE SERVICE TRAITS
// ============================================================================

/// Real-time incremental syntax highlighter bound to the visible viewport.
pub trait SyntaxHighlighter: Send {
    fn language(&self) -> LanguageId;

    /// Performs an initial parse of the buffer text.
    fn parse_full(&mut self, source: &str);

    /// Updates the internal syntax tree incrementally after a single buffer edit.
    fn apply_edit(&mut self, updated_source: &str, edit: &InputEditSummary);

    /// Queries strictly `viewport_bytes` (e.g. the 55 visible lines) and returns sorted spans.
    fn highlight_viewport(
        &mut self,
        source: &str,
        viewport_bytes: Range<usize>,
    ) -> Vec<HighlightSpan>;
}

/// Stateless background AST analyzer that drops the heavy AST before returning.
pub trait AstAnalyzer: Send + Sync {
    fn language(&self) -> LanguageId;

    /// Parses `source`, extracts lightweight symbols and diagnostics, and immediately drops the AST.
    fn analyze_ephemeral(&self, source: &str) -> AnalysisReport;

    /// Slices the minimal enclosing function/class and imports around `cursor_line` for AI prompts.
    fn extract_enclosing_context(
        &self,
        source: &str,
        cursor_line: usize,
    ) -> Option<EnclosingContext>;
}

/// Intercepts binary `.pyd` / `.so` module loads and routes them to human-readable stubs or sources.
pub trait BinaryStubInterceptor: Send + Sync {
    fn resolve_binary_module(
        &self,
        binary_path: &Path,
        sibling_files: &[PathBuf],
    ) -> StubResolution;
}

/// Errors that can occur during an in-memory code formatting pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// The buffer contains syntax errors and cannot be safely formatted.
    SyntaxError(String),
    /// Internal formatter failure.
    InternalError(String),
}

/// In-memory code formatter trait (implemented by `ruff_python_formatter` for Python,
/// and later `clang-format` / `rustfmt` for C/C++ and Rust).
pub trait CodeFormatter: Send + Sync {
    fn language(&self) -> LanguageId;

    /// Formats `source` in memory and returns the formatted string, or a `FormatError`
    /// if the source contains syntax errors.
    fn format_source(&self, source: &str) -> Result<String, FormatError>;
}

