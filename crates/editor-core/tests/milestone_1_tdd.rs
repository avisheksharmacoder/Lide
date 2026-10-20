use std::io::Cursor as IoCursor;
use std::time::Instant;
use editor_core::{MoveDirection, Position, TextBuffer};
use proptest::prelude::*;

/// Helper: Generates a realistic N-line Python source file (~42 bytes per line).
fn generate_python_source(total_lines: usize) -> String {
    let mut out = String::with_capacity(total_lines * 45);
    for i in 0..total_lines {
        match i % 5 {
            0 => out.push_str(&format!("class DataPipeline_{i}(BaseProcessor):\n")),
            1 => out.push_str("    def compute_metrics(self, tensor_x: list[float]) -> float:\n"),
            2 => out.push_str("        scaled = [val * 0.9981 for val in tensor_x if val >= 0.0]\n"),
            3 => out.push_str("        return sum(scaled) / max(len(scaled), 1)\n"),
            _ => out.push('\n'),
        }
    }
    out
}

// ============================================================================
// 1. 10,000-LINE PYTHON LOAD & LATENCY SLA TESTS
// ============================================================================

#[test]
fn test_load_10k_python_file_from_reader_under_5ms_sla() {
    let raw_python = generate_python_source(10_000);
    let expected_bytes = raw_python.len();
    let reader = IoCursor::new(raw_python.as_bytes());

    let start = Instant::now();
    let buffer = TextBuffer::from_reader(reader).expect("Failed to stream Python buffer");
    let elapsed = start.elapsed();

    // Note: 10,000 lines with trailing '\n' has 10,001 Rope lines (including trailing empty line)
    // or 10,000 logical lines depending on convention; we standardize on Rope's line count.
    assert!(buffer.len_lines() >= 10_000);
    assert_eq!(buffer.len_bytes(), expected_bytes);
    assert!(
        elapsed.as_millis() < 5,
        "10k-line load breached 5ms SLA: took {:?}",
        elapsed
    );
}

// ============================================================================
// 2. VIEWPORT VIRTUALIZATION & BYTE-RANGE SLICING TESTS
// ============================================================================

#[test]
fn test_viewport_slice_extracts_only_visible_55_lines_in_under_1ms() {
    let raw_python = generate_python_source(10_000);
    let buffer = TextBuffer::from_str(&raw_python);

    let start = Instant::now();
    // Simulate a 1080p Floem viewport scrolled halfway down the file (lines 5000..5055)
    let slice = buffer.viewport_slice(5_000, 5_055);
    let elapsed = start.elapsed();

    assert_eq!(slice.start_line, 5_000);
    assert_eq!(slice.end_line, 5_055);
    assert_eq!(slice.lines.len(), 55, "Must only materialize the 55 visible lines");

    // Verify first visible line in the slice (5000 % 5 == 0 -> class header)
    let first = &slice.lines[0];
    assert_eq!(first.line_idx, 5_000);
    assert_eq!(first.text, "class DataPipeline_5000(BaseProcessor):");
    assert!(!first.text.ends_with('\n'), "VisibleLine text must strip trailing newline");

    // Verify byte range matches exact underlying slice for Tree-sitter highlighting
    let expected_slice_bytes = &raw_python[slice.byte_range.clone()];
    assert!(expected_slice_bytes.starts_with("class DataPipeline_5000(BaseProcessor):\n"));

    assert!(
        elapsed.as_micros() < 1_000,
        "Viewport slicing breached 1ms SLA: took {:?}",
        elapsed
    );
}

#[test]
fn test_viewport_slice_clamps_gracefully_at_end_of_file() {
    let sample = "import os\nimport sys\n\ndef main():\n    pass";
    let buffer = TextBuffer::from_str(sample);

    // Request 50 lines starting at line 3 on a 5-line buffer
    let slice = buffer.viewport_slice(3, 53);

    assert_eq!(slice.start_line, 3);
    assert_eq!(slice.end_line, 5);
    assert_eq!(slice.lines.len(), 2);
    assert_eq!(slice.lines[0].text, "def main():");
    assert_eq!(slice.lines[1].text, "    pass");
}

#[test]
fn test_viewport_slice_handles_crlf_without_leaking_carriage_returns() {
    let crlf_python = "def alpha():\r\n    return 1\r\n";
    let buffer = TextBuffer::from_str(crlf_python);

    let slice = buffer.viewport_slice(0, 2);
    assert_eq!(slice.lines[0].text, "def alpha():");
    assert_eq!(slice.lines[1].text, "    return 1");
}

// ============================================================================
// 3. CURSOR PHYSICS, STICKY COLUMNS & UTF-8 GRAPHEME TESTS
// ============================================================================

#[test]
fn test_cursor_sticky_column_across_blank_python_lines() {
    let code = concat!(
        "    def compute_loss(self, weights: list[float]):\n", // len: 49 chars
        "\n",                                                   // len: 0 chars (blank line)
        "        return sum(weights) * 0.5\n"                   // len: 33 chars
    );
    let mut buffer = TextBuffer::from_str(code);

    // Place cursor at line 0, col 25
    buffer.set_cursor(Position::new(0, 25));
    assert_eq!(buffer.cursor().head, Position::new(0, 25));
    assert_eq!(buffer.cursor().preferred_col, 25);

    // Move Down onto the empty line (line 1): head.col must clamp to 0, but preferred_col stays 25
    buffer.move_cursor(MoveDirection::Down);
    assert_eq!(buffer.cursor().head, Position::new(1, 0));
    assert_eq!(buffer.cursor().preferred_col, 25);

    // Move Down again onto line 2: head.col must snap back to preferred_col (25)!
    buffer.move_cursor(MoveDirection::Down);
    assert_eq!(buffer.cursor().head, Position::new(2, 25));
}

#[test]
fn test_cursor_horizontal_wrapping_and_grapheme_boundaries() {
    // Line 0 contains multi-byte math symbols and a multi-codepoint emoji
    let code = "λ = \"👨\u{200d}👩\u{200d}👧\u{200d}👦\"\nx = 1";
    let mut buffer = TextBuffer::from_str(code);

    buffer.set_cursor(Position::new(0, 0));

    // Step right across: 'λ' (1), ' ' (2), '=' (3), ' ' (4), '"' (5), '👨‍👩‍👧‍👦' (6), '"' (7)
    for expected_col in 1..=7 {
        buffer.move_cursor(MoveDirection::Right);
        assert_eq!(buffer.cursor().head, Position::new(0, expected_col));
    }

    // Moving Right at the end of line 0 wraps to (line 1, col 0)
    buffer.move_cursor(MoveDirection::Right);
    assert_eq!(buffer.cursor().head, Position::new(1, 0));

    // Moving Left at (line 1, col 0) wraps back to the end of line 0 (col 7)
    buffer.move_cursor(MoveDirection::Left);
    assert_eq!(buffer.cursor().head, Position::new(0, 7));
}

// ============================================================================
// 4. EDITS & STRUCTURAL-SHARING UNDO / REDO TESTS
// ============================================================================

#[test]
fn test_insert_backspace_and_undo_redo_cycle() {
    let initial = "def greet():\n    pass";
    let mut buffer = TextBuffer::from_str(initial);

    // Move to end of line 1 ("    pass") and replace "pass" with "return True"
    buffer.set_cursor(Position::new(1, 8));
    for _ in 0..4 {
        buffer.delete_backward();
    }
    assert_eq!(buffer.to_string_lossy(), "def greet():\n    ");

    buffer.insert_at_cursor("return True");
    assert_eq!(buffer.to_string_lossy(), "def greet():\n    return True");
    assert_eq!(buffer.cursor().head, Position::new(1, 15));

    // Undo the insertion
    assert!(buffer.undo());
    assert_eq!(buffer.to_string_lossy(), "def greet():\n    ");

    // Redo the insertion
    assert!(buffer.redo());
    assert_eq!(buffer.to_string_lossy(), "def greet():\n    return True");
}

// ============================================================================
// 5. PROPERTY-BASED FUZZING (PROPTEST)
// ============================================================================

#[derive(Debug, Clone)]
enum FuzzOp {
    Insert(String),
    Backspace,
    Move(MoveDirection),
    Undo,
    Redo,
}

fn arb_fuzz_op() -> impl Strategy<Value = FuzzOp> {
    prop_oneof![
        "[a-zA-Z0-9_ \nλ🚀]{1,8}".prop_map(FuzzOp::Insert),
        Just(FuzzOp::Backspace),
        Just(FuzzOp::Move(MoveDirection::Up)),
        Just(FuzzOp::Move(MoveDirection::Down)),
        Just(FuzzOp::Move(MoveDirection::Left)),
        Just(FuzzOp::Move(MoveDirection::Right)),
        Just(FuzzOp::Undo),
        Just(FuzzOp::Redo),
    ]
}

proptest! {
    #[test]
    fn prop_random_editor_operations_never_panic_and_undo_restores_initial_state(
        initial_text in "[a-zA-Z0-9_ \n]{0,200}",
        ops in prop::collection::vec(arb_fuzz_op(), 1..60)
    ) {
        let mut buffer = TextBuffer::from_str(&initial_text);

        for op in ops {
            match op {
                FuzzOp::Insert(s) => buffer.insert_at_cursor(&s),
                FuzzOp::Backspace => buffer.delete_backward(),
                FuzzOp::Move(dir) => buffer.move_cursor(dir),
                FuzzOp::Undo => { buffer.undo(); }
                FuzzOp::Redo => { buffer.redo(); }
            }

            // Invariant 1: Cursor line is always within valid Rope bounds
            let pos = buffer.cursor().head;
            prop_assert!(pos.line < buffer.len_lines());

            // Invariant 2: Requesting a 50-line viewport at the cursor never panics
            let _slice = buffer.viewport_slice(pos.line, pos.line + 50);
        }

        // Invariant 3: Draining the entire undo stack always restores `initial_text`
        while buffer.undo() {}
        prop_assert_eq!(buffer.to_string_lossy(), initial_text);
    }
}
