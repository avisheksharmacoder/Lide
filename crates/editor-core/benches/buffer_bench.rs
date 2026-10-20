use std::io::Cursor as IoCursor;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use editor_core::{Position, TextBuffer};

fn build_10k_python_string() -> String {
    (0..10_000)
        .map(|i| format!("def func_{i}(x: int, y: int) -> int: return x + y * {i}\n"))
        .collect()
}

fn bench_editor_core(c: &mut Criterion) {
    let raw_10k = build_10k_python_string();

    c.bench_function("load_10k_python_lines_from_reader", |b| {
        b.iter(|| {
            let reader = IoCursor::new(black_box(raw_10k.as_bytes()));
            TextBuffer::from_reader(reader).unwrap()
        })
    });

    let buffer = TextBuffer::from_str(&raw_10k);
    c.bench_function("viewport_slice_55_lines_at_line_5000", |b| {
        b.iter(|| {
            buffer.viewport_slice(black_box(5_000), black_box(5_055))
        })
    });

    c.bench_function("insert_char_at_line_5000_with_undo_snapshot", |b| {
        let mut editable = TextBuffer::from_str(&raw_10k);
        editable.set_cursor(Position::new(5_000, 12));
        b.iter(|| {
            editable.insert_at_cursor(black_box("a"));
            editable.undo();
        })
    });
}

criterion_group!(benches, bench_editor_core);
criterion_main!(benches);
