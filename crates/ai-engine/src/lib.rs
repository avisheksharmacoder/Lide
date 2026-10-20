use std::sync::Arc;

use lang_contract::{AstAnalyzer, EnclosingContext};
use lang_python::PythonAstAnalyzer;
use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::models::ModelOptions;
use ollama_rs::Ollama;
use tokio::sync::{mpsc, watch};
use tokio_stream::StreamExt;

/// Specialized Python coding tasks supported by the local AI assistant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiTaskKind {
    /// Completes the body or next lines of the active function/method.
    InlineCompletion,
    /// Refactors the enclosing function according to a custom user instruction.
    Refactor { instruction: String },
    /// Generates a PEP 257 docstring and type annotations for the active symbol.
    GenerateDocstring,
    /// Generates a compact `pytest` unit test for the active function/method.
    WritePytest,
}

/// A compact, AST-pruned prompt payload ready to send to Ollama.
#[derive(Debug, Clone, PartialEq)]
pub struct AiPromptRequest {
    pub model: String,
    pub task: AiTaskKind,
    pub context: EnclosingContext,
    pub formatted_prompt: String,
    pub temperature: f32,
}

/// Real-time events emitted over an `mpsc` channel to Floem UI signals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiStreamEvent {
    /// A single token or chunk arrived from the model.
    Token(String),
    /// Stream finished cleanly; contains the full accumulated text and stripped code block.
    Completed {
        raw_response: String,
        clean_python_code: String,
    },
    /// Stream was aborted mid-flight because the user kept typing or pressed Escape.
    Cancelled,
    /// Model or connection error (e.g. Ollama daemon not running on port 11434).
    Error(String),
}

/// Handle returned to the UI thread to read streaming tokens or abort an in-flight request.
pub struct AiStreamHandle {
    pub rx: mpsc::Receiver<AiStreamEvent>,
    cancel_tx: watch::Sender<bool>,
}

impl AiStreamHandle {
    /// Signals the background Tokio streaming task to abort immediately.
    pub fn cancel(&self) {
        let _ = self.cancel_tx.send(true);
    }

    /// Returns a cloneable cancellation sender so the UI view-model can abort the task
    /// even after `self.rx` is moved into Floem's async event bridge.
    pub fn cancel_sender(&self) -> watch::Sender<bool> {
        self.cancel_tx.clone()
    }
}

/// Mockable trait abstracting token streaming from an LLM backend (`ollama-rs` or deterministic test mocks).
pub trait LlmStreamingClient: Send + Sync + 'static {
    /// Spawns or executes a token stream, pushing raw string chunks into `chunk_tx`
    /// until completion or until `cancel_rx` becomes `true`.
    fn stream_completion(
        &self,
        request: AiPromptRequest,
        chunk_tx: mpsc::Sender<Result<String, String>>,
        cancel_rx: watch::Receiver<bool>,
    );
}

// ============================================================================
// 1. AST-SLICED PROMPT BUILDER
// ============================================================================

/// Builds surgical, token-efficient prompts from a Python buffer using `PythonAstAnalyzer`.
#[derive(Default)]
pub struct AstPromptBuilder {
    analyzer: PythonAstAnalyzer,
}

impl AstPromptBuilder {
    pub fn new() -> Self {
        Self {
            analyzer: PythonAstAnalyzer::new(),
        }
    }

    /// Extracts the `EnclosingContext` around `cursor_line` from a full Python buffer
    /// and formats a compact `AiPromptRequest` for the specified `model` and `task`.
    pub fn build_from_source(
        &self,
        source: &str,
        cursor_line: usize,
        model: &str,
        task: AiTaskKind,
    ) -> Option<AiPromptRequest> {
        let context = self.analyzer.extract_enclosing_context(source, cursor_line)?;
        let formatted_prompt = Self::format_prompt(&context, &task);

        let temperature = match task {
            AiTaskKind::InlineCompletion | AiTaskKind::Refactor { .. } => 0.1,
            AiTaskKind::GenerateDocstring | AiTaskKind::WritePytest => 0.2,
        };

        Some(AiPromptRequest {
            model: model.to_string(),
            task,
            context,
            formatted_prompt,
            temperature,
        })
    }

    fn format_prompt(ctx: &EnclosingContext, task: &AiTaskKind) -> String {
        let imports_section = if ctx.module_imports.is_empty() {
            String::new()
        } else {
            format!("# Module Imports:\n{}\n\n", ctx.module_imports.join("\n"))
        };

        let task_instruction = match task {
            AiTaskKind::InlineCompletion => {
                "Complete or optimize the following Python code. Output ONLY valid Python code."
                    .to_string()
            }
            AiTaskKind::Refactor { instruction } => {
                format!("Refactor the following Python code ({instruction}). Output ONLY valid Python code.")
            }
            AiTaskKind::GenerateDocstring => {
                "Add a concise PEP 257 docstring and complete type hints to the following Python symbol. Output ONLY valid Python code."
                    .to_string()
            }
            AiTaskKind::WritePytest => {
                "Write a concise `pytest` unit test function for the following Python symbol. Output ONLY valid Python code."
                    .to_string()
            }
        };

        format!(
            "{task_instruction}\n\n{imports_section}# Active Symbol ({}):\n{}\n",
            ctx.symbol_name, ctx.enclosing_source
        )
    }
}

// ============================================================================
// 2. CANCELLATION-SAFE STREAMING COORDINATOR
// ============================================================================

/// Coordinates background Tokio streaming tasks, markdown fence stripping, and cancellation.
pub struct AiEngine<C: LlmStreamingClient> {
    client: Arc<C>,
}

impl<C: LlmStreamingClient> AiEngine<C> {
    pub fn new(client: C) -> Self {
        Self {
            client: Arc::new(client),
        }
    }

    /// Starts a non-blocking streaming session on the Tokio runtime and returns an `AiStreamHandle`.
    pub fn start_stream(&self, request: AiPromptRequest) -> AiStreamHandle {
        let (event_tx, event_rx) = mpsc::channel::<AiStreamEvent>(64);
        let (chunk_tx, mut chunk_rx) = mpsc::channel::<Result<String, String>>(64);
        let (cancel_tx, mut cancel_rx) = watch::channel(false);

        let client_cancel_rx = cancel_rx.clone();
        self.client
            .stream_completion(request, chunk_tx, client_cancel_rx);

        tokio::spawn(async move {
            let mut accumulated = String::new();

            loop {
                tokio::select! {
                    biased;

                    changed = cancel_rx.changed() => {
                        if changed.is_ok() && *cancel_rx.borrow() {
                            let _ = event_tx.send(AiStreamEvent::Cancelled).await;
                            return;
                        }
                    }

                    maybe_chunk = chunk_rx.recv() => {
                        match maybe_chunk {
                            Some(Ok(token)) => {
                                if *cancel_rx.borrow() {
                                    let _ = event_tx.send(AiStreamEvent::Cancelled).await;
                                    return;
                                }
                                accumulated.push_str(&token);
                                if event_tx.send(AiStreamEvent::Token(token)).await.is_err() {
                                    return;
                                }
                            }
                            Some(Err(err_msg)) => {
                                let _ = event_tx.send(AiStreamEvent::Error(err_msg)).await;
                                return;
                            }
                            None => {
                                if *cancel_rx.borrow() {
                                    let _ = event_tx.send(AiStreamEvent::Cancelled).await;
                                } else {
                                    let clean_python_code = extract_clean_python_code(&accumulated);
                                    let _ = event_tx
                                        .send(AiStreamEvent::Completed {
                                            raw_response: accumulated,
                                            clean_python_code,
                                        })
                                        .await;
                                }
                                return;
                            }
                        }
                    }
                }
            }
        });

        AiStreamHandle {
            rx: event_rx,
            cancel_tx,
        }
    }
}

// ============================================================================
// 3. PRODUCTION OLLAMA-RS STREAMING CLIENT
// ============================================================================

/// Production `LlmStreamingClient` backed by `ollama_rs::Ollama::generate_stream`.
pub struct OllamaStreamingClient {
    ollama: Ollama,
}

impl OllamaStreamingClient {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            ollama: Ollama::builder().host(host.to_string()).port(port).build(),
        }
    }
}

impl Default for OllamaStreamingClient {
    fn default() -> Self {
        Self::new("http://localhost", 11434)
    }
}

impl LlmStreamingClient for OllamaStreamingClient {
    fn stream_completion(
        &self,
        request: AiPromptRequest,
        chunk_tx: mpsc::Sender<Result<String, String>>,
        mut cancel_rx: watch::Receiver<bool>,
    ) {
        let ollama = self.ollama.clone();

        tokio::spawn(async move {
            let options = ModelOptions::default().temperature(request.temperature);
            let gen_req =
                GenerationRequest::new(request.model, request.formatted_prompt).options(options);

            let mut stream = match ollama.generate_stream(gen_req).await {
                Ok(s) => s,
                Err(err) => {
                    let _ = chunk_tx
                        .send(Err(format!("Ollama connection error: {err}")))
                        .await;
                    return;
                }
            };

            loop {
                tokio::select! {
                    biased;

                    _ = cancel_rx.changed() => {
                        if *cancel_rx.borrow() {
                            break;
                        }
                    }

                    next_item = stream.next() => {
                        let Some(res) = next_item else {
                            break;
                        };
                        match res {
                            Ok(responses) => {
                                for resp in responses {
                                    if *cancel_rx.borrow() {
                                        return;
                                    }
                                    if !resp.response.is_empty()
                                        && chunk_tx.send(Ok(resp.response)).await.is_err()
                                    {
                                        return;
                                    }
                                }
                            }
                            Err(err) => {
                                let _ = chunk_tx
                                    .send(Err(format!("Ollama stream error: {err}")))
                                    .await;
                                return;
                            }
                        }
                    }
                }
            }
        });
    }
}

// ============================================================================
// 4. MARKDOWN CODE-FENCE SANITIZER
// ============================================================================

/// Strips surrounding ```python ... ``` or ``` ... ``` markdown fences if an LLM wraps its code output.
pub fn extract_clean_python_code(raw: &str) -> String {
    let trimmed = raw.trim();

    if let Some(fence_start) = trimmed.find("```") {
        let after_opening_ticks = &trimmed[fence_start + 3..];
        // Skip optional language identifier on the opening fence line (e.g. `python\n`)
        let code_start = after_opening_ticks
            .find('\n')
            .map(|idx| idx + 1)
            .unwrap_or(0);
        let body = &after_opening_ticks[code_start..];

        if let Some(fence_end) = body.find("```") {
            return body[..fence_end].trim_end().to_string();
        }
    }

    trimmed.to_string()
}
