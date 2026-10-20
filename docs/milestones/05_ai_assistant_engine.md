# Milestone 5: AST-Aware Local AI Assistant (`ai-engine`)

> **Goal**: Build a privacy-first, local AI assistant integration powered by Ollama. Use AST-guided context extraction to prune prompt tokens, stream code completions into inline ghost-text and a right-hand chat split pane, and guarantee 100% testability through a deterministic mock streaming engine.

---

## 1. Scope & Crate Responsibility

- **Crate**: `crates/ai-engine`
- **Dependencies Allowed**:
  - `lang-contract`
  - `tokio` (Async runtime for streaming HTTP/SSE)
  - `ollama-rs` (Official/standard Ollama client crate)
  - `async-trait` (Ergonomic async traits for mockable client)
  - `serde`, `serde_json`
  - `tokio-test` (Dev dependency for testing async streams)

---

## 2. Core Architecture & Mockable Client

### 2.1 The `AiClient` Mockable Trait
To ensure unit tests never require a live Ollama daemon running, all streaming interacts with a trait:

```rust
#[async_trait]
pub trait AiClient: Send + Sync {
    /// Stream code completion tokens for an inline prompt.
    async fn stream_completion(
        &self,
        prompt: &str,
        cancel: tokio_util::sync::CancellationToken,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, AiError>> + Send>>, AiError>;

    /// Generate an explanation or refactor response in the assistant pane.
    async fn stream_chat(
        &self,
        messages: Vec<ChatMessage>,
        cancel: tokio_util::sync::CancellationToken,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, AiError>> + Send>>, AiError>;
}
```

### 2.2 AST-Aware Prompt Slicing
Instead of sending thousands of lines of unneeded buffer code, the AST engine extracts targeted context:

```rust
pub struct CodeContext {
    pub file_imports: String,
    pub enclosing_class_signature: Option<String>,
    pub enclosing_function_body: String,
    pub prefix_lines: String,    // ~20 lines immediately before cursor
    pub suffix_lines: String,    // ~10 lines immediately after cursor
    pub active_diagnostics: Vec<Diagnostic>,
}
```

---

## 3. Required Implementation Tasks

### Task 5.1: AST-Aware Context Extractor
- Using `tree-sitter-python` node queries:
  - Locate the node enclosing the current cursor offset.
  - Traverse up to identify the nearest `FunctionDefinition` and `ClassDefinition`.
  - Extract all top-level `import` and `from ... import` statements.
  - Bundle extracted elements into a concise `CodeContext` prompt payload.
  - Ensures local 7B/8B models (e.g. `qwen2.5-coder`, `deepseek-coder`, `llama3`) stay well within their fast context windows.

### Task 5.2: Stream Cancellation on User Keystrokes
- When user pauses for 400ms, trigger inline completion request with a `CancellationToken`.
- If the user types any character, moves the cursor, or presses `Esc`:
  - Instantly trigger `cancel.cancel()`.
  - Terminate the Ollama HTTP stream immediately to prevent CPU/GPU saturation on the host.

### Task 5.3: Inline Ghost-Text Overlay (`app-floem`)
- Floem canvas overlay:
  - If an inline completion stream produces suggested text (e.g. `) -> bool:` or a multi-line loop body), render the text in muted italic gray directly following the cursor.
  - Pressing `Tab` accepts the suggestion and commits it into `editor-core`.
  - Pressing `Esc` or typing rejects the suggestion.

### Task 5.4: Collapsible AI Assistant Split Pane
- Right-hand side panel in Floem:
  - Markdown-rendered chat stream.
  - Action buttons: "Explain this function", "Generate unit test with pytest", "Refactor with types".
  - Code blocks with one-click "Insert at Cursor" or "Replace Selection".

---

## 4. Test-Driven Development (TDD) Plan

### Tier 5.1: Deterministic Mock Stream Test
```rust
struct MockAiClient {
    tokens_to_yield: Vec<&'static str>,
}

#[tokio::test]
async fn test_mock_streaming_accumulation_and_cancellation() {
    let mock = MockAiClient {
        tokens_to_yield: vec!["def ", "add(a, ", "b):", "\n    return a + b"],
    };
    let cancel = CancellationToken::new();
    let mut stream = mock.stream_completion("prompt", cancel.clone()).await.unwrap();
    
    let mut accumulated = String::new();
    while let Some(Ok(token)) = stream.next().await {
        accumulated.push_str(&token);
        if accumulated.contains("b):") {
            cancel.cancel(); // Simulate user hitting Esc mid-stream
            break;
        }
    }
    
    assert_eq!(accumulated, "def add(a, b):");
    assert!(cancel.is_cancelled());
}
```

### Tier 5.2: AST Context Extraction Unit Test
- Feed a sample Python file with 200 lines containing multiple classes and helper functions.
- Place cursor inside `def calculate_metric(...)`.
- Extract context:
  - Assert `file_imports` contains `import math`.
  - Assert `enclosing_function_body` contains `def calculate_metric`.
  - Assert unrelated functions in the file are omitted from the extracted context.

---

## 5. Acceptance Criteria Checklist

- [ ] Deterministic unit tests verify prompt assembly and token streaming without Ollama installed.
- [ ] Typing any key during an active generation immediately cancels the async task within 10ms.
- [ ] Inline ghost-text renders accurately without shifting active buffer text coordinates.
- [ ] Pressing `Tab` accepts ghost-text and integrates cleanly into the undo/redo tree.
- [ ] Right-hand assistant split pane opens/closes with zero frame lag.
