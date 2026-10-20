use std::time::Duration;
use ai_engine::{
    extract_clean_python_code, AiEngine, AiPromptRequest, AiStreamEvent, AiTaskKind,
    AstPromptBuilder, LlmStreamingClient,
};
use tokio::sync::{mpsc, watch};

/// Helper: Generates a 10,000-line Python file where each class has a distinct index.
fn generate_10k_python_buffer() -> String {
    let mut out = String::with_capacity(10_000 * 48);
    out.push_str("import math\nfrom typing import Sequence\n\n");
    let mut line_count = 3;
    let mut idx = 0;

    while line_count < 10_000 {
        out.push_str(&format!(
            "class BlockProcessor_{idx}:\n    def transform(self, items: Sequence[float]) -> float:\n        return sum(items) * {idx}\n\n"
        ));
        line_count += 4;
        idx += 1;
    }
    out
}

/// Deterministic mock LLM client for fast, offline TDD verification.
struct MockStreamingClient {
    chunks: Vec<String>,
    delay_per_chunk_ms: u64,
}

impl LlmStreamingClient for MockStreamingClient {
    fn stream_completion(
        &self,
        _request: AiPromptRequest,
        chunk_tx: mpsc::Sender<Result<String, String>>,
        mut cancel_rx: watch::Receiver<bool>,
    ) {
        let chunks = self.chunks.clone();
        let delay = self.delay_per_chunk_ms;

        tokio::spawn(async move {
            for chunk in chunks {
                if *cancel_rx.borrow() {
                    break;
                }
                if delay > 0 {
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(delay)) => {}
                        _ = cancel_rx.changed() => {
                            if *cancel_rx.borrow() {
                                break;
                            }
                        }
                    }
                }
                if chunk_tx.send(Ok(chunk)).await.is_err() {
                    break;
                }
            }
        });
    }
}

// ============================================================================
// 1. AST-SLICED PROMPT PRUNING ON A 10,000-LINE PYTHON BUFFER
// ============================================================================

#[test]
fn test_ast_prompt_builder_prunes_10k_line_buffer_to_only_active_symbol_and_imports() {
    let full_source = generate_10k_python_buffer();
    let builder = AstPromptBuilder::new();

    // Place cursor at `BlockProcessor_1200::transform` (line 3 + 1200 * 4 + 1 = 4804)
    let cursor_line = 4_804;
    let req = builder
        .build_from_source(
            &full_source,
            cursor_line,
            "qwen2.5-coder:7b",
            AiTaskKind::Refactor {
                instruction: "Convert sum() to math.fsum() for floating-point precision".to_string(),
            },
        )
        .expect("Should extract enclosing AST context at line 4804");

    assert_eq!(req.model, "qwen2.5-coder:7b");
    assert_eq!(req.context.symbol_name, "BlockProcessor_1200");

    // 1. Prompt must include top-level module imports
    assert!(req.formatted_prompt.contains("import math"));
    assert!(req.formatted_prompt.contains("from typing import Sequence"));

    // 2. Prompt must include the target class `BlockProcessor_1200`
    assert!(req.formatted_prompt.contains("class BlockProcessor_1200:"));

    // 3. Prompt must NOT include unrelated classes from the 10,000-line file
    assert!(
        !req.formatted_prompt.contains("BlockProcessor_0"),
        "Prompt leaked unrelated top-of-file class"
    );
    assert!(
        !req.formatted_prompt.contains("BlockProcessor_2000"),
        "Prompt leaked unrelated bottom-of-file class"
    );

    // 4. Total prompt size must be tiny (< 600 bytes instead of ~450,000 bytes)
    assert!(
        req.formatted_prompt.len() < 600,
        "Expected compact AST-sliced prompt (<600 bytes), got {} bytes",
        req.formatted_prompt.len()
    );
}

// ============================================================================
// 2. ASYNC TOKEN STREAMING & MARKDOWN CODE-FENCE SANITIZATION
// ============================================================================

#[tokio::test]
async fn test_ai_engine_streams_tokens_in_order_and_strips_markdown_fences_on_completion() {
    let sample_py = "import math\n\ndef area(r: float) -> float:\n    return math.pi * r * r\n";
    let builder = AstPromptBuilder::new();
    let req = builder
        .build_from_source(sample_py, 3, "qwen2.5-coder:7b", AiTaskKind::GenerateDocstring)
        .unwrap();

    let mock_client = MockStreamingClient {
        chunks: vec![
            "```python\n".to_string(),
            "def area(r: float) -> float:\n".to_string(),
            "    \"\"\"Compute circle area.\"\"\"\n".to_string(),
            "    return math.pi * (r ** 2)\n".to_string(),
            "```".to_string(),
        ],
        delay_per_chunk_ms: 0,
    };

    let engine = AiEngine::new(mock_client);
    let mut handle = engine.start_stream(req);

    let mut collected_tokens = Vec::new();
    let mut final_clean_code = None;

    while let Some(event) = handle.rx.recv().await {
        match event {
            AiStreamEvent::Token(tok) => collected_tokens.push(tok),
            AiStreamEvent::Completed {
                clean_python_code, ..
            } => {
                final_clean_code = Some(clean_python_code);
                break;
            }
            other => panic!("Unexpected stream event: {:?}", other),
        }
    }

    assert_eq!(collected_tokens.len(), 5);
    assert_eq!(
        final_clean_code.as_deref(),
        Some("def area(r: float) -> float:\n    \"\"\"Compute circle area.\"\"\"\n    return math.pi * (r ** 2)")
    );
}

// ============================================================================
// 3. MID-STREAM CANCELLATION SAFETY (WHEN USER KEEPS TYPING OR PRESSES ESC)
// ============================================================================

#[tokio::test]
async fn test_ai_stream_cancels_immediately_when_handle_cancel_is_triggered() {
    let sample_py = "def slow_fn(x: int) -> int:\n    return x\n";
    let builder = AstPromptBuilder::new();
    let req = builder
        .build_from_source(sample_py, 1, "qwen2.5-coder:7b", AiTaskKind::InlineCompletion)
        .unwrap();

    // Simulate a slow stream (50ms per token, 10 tokens total = 500ms if uncanceled)
    let mock_client = MockStreamingClient {
        chunks: (0..10).map(|i| format!("token_{i} ")).collect(),
        delay_per_chunk_ms: 50,
    };

    let engine = AiEngine::new(mock_client);
    let mut handle = engine.start_stream(req);

    // Receive the first token, then immediately cancel (simulating a user keystroke)
    let first = handle.rx.recv().await.expect("Should receive first token");
    assert!(matches!(first, AiStreamEvent::Token(_)));

    handle.cancel();

    // Must receive `AiStreamEvent::Cancelled` and close the channel without emitting all 10 tokens
    let mut saw_cancelled = false;
    let mut token_count = 1;
    while let Some(event) = handle.rx.recv().await {
        match event {
            AiStreamEvent::Token(_) => token_count += 1,
            AiStreamEvent::Cancelled => {
                saw_cancelled = true;
                break;
            }
            other => panic!("Expected Cancelled event, got: {:?}", other),
        }
    }

    assert!(saw_cancelled, "Stream must emit AiStreamEvent::Cancelled on abort");
    assert!(
        token_count < 5,
        "Cancelled stream leaked too many tokens ({token_count}) after abort"
    );
}

// ============================================================================
// 4. DIRECT UNIT TEST FOR MARKDOWN FENCE SANITIZER
// ============================================================================

#[test]
fn test_extract_clean_python_code_handles_fenced_and_unfenced_snippets() {
    let fenced = "Here is the refactored code:\n```python\ndef add(a: int, b: int) -> int:\n    return a + b\n```\nHope this helps!";
    assert_eq!(
        extract_clean_python_code(fenced),
        "def add(a: int, b: int) -> int:\n    return a + b"
    );

    let plain = "def add(a: int, b: int) -> int:\n    return a + b\n";
    assert_eq!(
        extract_clean_python_code(plain),
        "def add(a: int, b: int) -> int:\n    return a + b"
    );
}
