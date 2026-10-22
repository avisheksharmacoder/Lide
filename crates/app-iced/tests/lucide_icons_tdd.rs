//! TDD test verifying Lucide icon font embedding and widget generation.

use app_iced::icons::{icon, icon_text, Icon};
use lucide_icons::LUCIDE_FONT_BYTES;

#[test]
fn test_lucide_font_bytes_embedded_and_valid_ttf() {
    assert!(
        LUCIDE_FONT_BYTES.len() > 10_000,
        "Lucide font bytes must be non-empty and bundled in binary"
    );

    // Verify TTF magic number: 0x00010000 or 'OTTO'
    assert!(
        LUCIDE_FONT_BYTES.starts_with(&[0x00, 0x01, 0x00, 0x00])
            || LUCIDE_FONT_BYTES.starts_with(b"OTTO"),
        "Lucide font must have valid TTF/OTF magic header"
    );
}

#[test]
fn test_lucide_icon_conversion_and_codepoint_resolution() {
    // Verify key IDE icon glyph codepoints resolve to valid unicode chars
    let icons = [
        Icon::Folder,
        Icon::FileCode,
        Icon::Save,
        Icon::Search,
        Icon::FlaskConical,
        Icon::Sparkles,
        Icon::Plus,
        Icon::Zap,
        Icon::Play,
        Icon::Square,
        Icon::Check,
        Icon::X,
        Icon::Settings,
        Icon::Type,
        Icon::Palette,
        Icon::Sliders,
        Icon::Cpu,
        Icon::Moon,
        Icon::Sun,
    ];

    for ic in icons {
        let ch = char::from(ic);
        assert!(
            ch as u32 > 0,
            "Icon variant {ic:?} must map to a valid positive Unicode codepoint"
        );

        let _text = icon_text(ic);
        let _elem: iced::Element<'_, ()> = icon(ic);
    }
}

#[test]
fn test_parse_lucide_ttf_font_family_name() {
    let bytes = LUCIDE_FONT_BYTES;
    assert!(bytes.len() > 12);
    let num_tables = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    let mut name_offset = None;

    for i in 0..num_tables {
        let base = 12 + i * 16;
        if base + 16 > bytes.len() { break; }
        let tag = &bytes[base..base + 4];
        if tag == b"name" {
            let offset = u32::from_be_bytes([bytes[base + 8], bytes[base + 9], bytes[base + 10], bytes[base + 11]]) as usize;
            name_offset = Some(offset);
            break;
        }
    }

    let Some(name_off) = name_offset else { panic!("name table not found"); };
    let count = u16::from_be_bytes([bytes[name_off + 2], bytes[name_off + 3]]) as usize;
    let string_offset = name_off + u16::from_be_bytes([bytes[name_off + 4], bytes[name_off + 5]]) as usize;

    let mut names = Vec::new();
    for i in 0..count {
        let rec = name_off + 6 + i * 12;
        let name_id = u16::from_be_bytes([bytes[rec + 6], bytes[rec + 7]]);
        let length = u16::from_be_bytes([bytes[rec + 8], bytes[rec + 9]]) as usize;
        let offset = string_offset + u16::from_be_bytes([bytes[rec + 10], bytes[rec + 11]]) as usize;
        if offset + length <= bytes.len() && (name_id == 1 || name_id == 4) {
            let slice = &bytes[offset..offset + length];
            let s = if slice.contains(&0) {
                // UTF-16BE
                let u16s: Vec<u16> = slice.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
                String::from_utf16_lossy(&u16s)
            } else {
                String::from_utf8_lossy(slice).to_string()
            };
            names.push((name_id, s));
        }
    }
    assert!(!names.is_empty(), "Must find font family names");
    let has_lucide = names.iter().any(|(_, n)| n.to_lowercase().contains("lucide"));
    assert!(has_lucide, "Font family must be Lucide: {:?}", names);
}
