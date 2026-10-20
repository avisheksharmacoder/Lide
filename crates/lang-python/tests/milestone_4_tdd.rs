use std::time::Instant;
use lang_contract::{CodeFormatter, FormatError};
use lang_python::PythonFormatter;

#[test]
fn test_ruff_formatter_normalizes_messy_python_code_under_5ms() {
    let messy_python = concat!(
        "import sys,os\n",
        "def   calc_weights( x:int,y:float=1.5 )->float:\n",
        "    items=[1,2,   3,4]\n",
        "    return   (x+y)*len( items )\n"
    );

    let formatter = PythonFormatter::new();
    let start = Instant::now();
    let formatted = formatter
        .format_source(messy_python)
        .expect("Valid Python code should format cleanly");
    let elapsed = start.elapsed();

    // Verify Ruff canonical formatting rules: normalized spacing around operators, colons, and args
    assert!(
        formatted.contains("def calc_weights(x: int, y: float = 1.5) -> float:"),
        "Function signature was not normalized by Ruff:\n{formatted}"
    );
    assert!(
        formatted.contains("items = [1, 2, 3, 4]"),
        "List literal spacing was not normalized:\n{formatted}"
    );
    assert!(
        formatted.contains("return (x + y) * len(items)"),
        "Return expression spacing was not normalized:\n{formatted}"
    );

    assert!(
        elapsed.as_millis() < 5,
        "In-memory Ruff formatting breached 5ms SLA: took {:?}",
        elapsed
    );
}

#[test]
fn test_ruff_formatter_is_idempotent() {
    let messy = "x=1+2\ny = [ 'a','b' ]\n";
    let formatter = PythonFormatter::new();

    let pass_one = formatter.format_source(messy).unwrap();
    let pass_two = formatter.format_source(&pass_one).unwrap();

    assert_eq!(pass_one, pass_two, "Ruff formatting must be strictly idempotent");
}

#[test]
fn test_ruff_formatter_rejects_syntax_errors_without_corrupting_source() {
    let broken_python = "def incomplete_func(a: int, b:\n    return a\n";
    let formatter = PythonFormatter::new();

    let result = formatter.format_source(broken_python);
    match result {
        Err(FormatError::SyntaxError(msg)) => {
            assert!(!msg.is_empty(), "SyntaxError message should not be empty");
        }
        other => panic!("Expected Err(FormatError::SyntaxError), got: {:?}", other),
    }
}
