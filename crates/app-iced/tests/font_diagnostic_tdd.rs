//! Diagnostic test to inspect TrueType name tables of bundled fonts.

use app_iced::fonts::{JETBRAINS_MONO_BOLD_BYTES, JETBRAINS_MONO_MEDIUM_BYTES};

fn get_name_records(ttf_bytes: &[u8]) -> Vec<(u16, u16, u16, u16, String)> {
    if ttf_bytes.len() < 12 {
        return Vec::new();
    }
    let num_tables = u16::from_be_bytes([ttf_bytes[4], ttf_bytes[5]]) as usize;
    let mut name_offset = None;

    for i in 0..num_tables {
        let entry = 12 + i * 16;
        if entry + 16 > ttf_bytes.len() {
            break;
        }
        let tag = &ttf_bytes[entry..entry + 4];
        if tag == b"name" {
            let offset = u32::from_be_bytes([
                ttf_bytes[entry + 8],
                ttf_bytes[entry + 9],
                ttf_bytes[entry + 10],
                ttf_bytes[entry + 11],
            ]) as usize;
            name_offset = Some(offset);
            break;
        }
    }

    let Some(name_off) = name_offset else {
        return Vec::new();
    };

    if name_off + 6 > ttf_bytes.len() {
        return Vec::new();
    }

    let count = u16::from_be_bytes([ttf_bytes[name_off + 2], ttf_bytes[name_off + 3]]) as usize;
    let string_storage_offset =
        name_off + u16::from_be_bytes([ttf_bytes[name_off + 4], ttf_bytes[name_off + 5]]) as usize;

    let mut results = Vec::new();
    for i in 0..count {
        let rec = name_off + 6 + i * 12;
        if rec + 12 > ttf_bytes.len() {
            break;
        }
        let platform_id = u16::from_be_bytes([ttf_bytes[rec], ttf_bytes[rec + 1]]);
        let encoding_id = u16::from_be_bytes([ttf_bytes[rec + 2], ttf_bytes[rec + 3]]);
        let language_id = u16::from_be_bytes([ttf_bytes[rec + 4], ttf_bytes[rec + 5]]);
        let name_id = u16::from_be_bytes([ttf_bytes[rec + 6], ttf_bytes[rec + 7]]);
        let length = u16::from_be_bytes([ttf_bytes[rec + 8], ttf_bytes[rec + 9]]) as usize;
        let str_offset = u16::from_be_bytes([ttf_bytes[rec + 10], ttf_bytes[rec + 11]]) as usize;

        let str_start = string_storage_offset + str_offset;
        if str_start + length <= ttf_bytes.len() {
            let slice = &ttf_bytes[str_start..str_start + length];
            let val = if platform_id == 3 || platform_id == 0 {
                // UTF-16BE
                let u16s: Vec<u16> = slice
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                String::from_utf16_lossy(&u16s)
            } else {
                String::from_utf8_lossy(slice).to_string()
            };
            results.push((platform_id, encoding_id, language_id, name_id, val));
        }
    }
    results
}

#[test]
fn test_inspect_font_names() {
    let medium_names = get_name_records(JETBRAINS_MONO_MEDIUM_BYTES);
    println!("=== JETBRAINS MONO MEDIUM NAMES ===");
    for (plat, enc, lang, id, val) in &medium_names {
        if *id == 1 || *id == 2 || *id == 4 || *id == 16 || *id == 17 {
            println!("ID {id} (plat {plat}, enc {enc}, lang {lang}): '{val}'");
        }
    }

    let bold_names = get_name_records(JETBRAINS_MONO_BOLD_BYTES);
    println!("=== JETBRAINS MONO BOLD NAMES ===");
    for (plat, enc, lang, id, val) in &bold_names {
        if *id == 1 || *id == 2 || *id == 4 || *id == 16 || *id == 17 {
            println!("ID {id} (plat {plat}, enc {enc}, lang {lang}): '{val}'");
        }
    }
}
