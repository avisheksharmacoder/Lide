//! Bundled TrueType fonts embedded directly into the Lide binary.
//!
//! Provides zero-network, zero-download font loading for JetBrains Mono.

/// Represents an embedded TrueType font resource bundled at compile time.
pub struct BundledFont {
    pub filename: &'static str,
    pub data: &'static [u8],
}

/// The collection of JetBrains Mono fonts bundled into the executable.
pub static BUNDLED_FONTS: &[BundledFont] = &[
    BundledFont {
        filename: "JetBrainsMono-Medium.ttf",
        data: include_bytes!("../../../fonts/JetBrainsMono-Medium.ttf"),
    },
    BundledFont {
        filename: "JetBrainsMono-Bold.ttf",
        data: include_bytes!("../../../fonts/JetBrainsMono-Bold.ttf"),
    },
    BundledFont {
        filename: "JetBrainsMono-Italic.ttf",
        data: include_bytes!("../../../fonts/JetBrainsMono-Italic.ttf"),
    },
    BundledFont {
        filename: "JetBrainsMono-BoldItalic.ttf",
        data: include_bytes!("../../../fonts/JetBrainsMono-BoldItalic.ttf"),
    },
    BundledFont {
        filename: "JetBrainsMono-ExtraBold.ttf",
        data: include_bytes!("../../../fonts/JetBrainsMono-ExtraBold.ttf"),
    },
    BundledFont {
        filename: "JetBrainsMono-ExtraBoldItalic.ttf",
        data: include_bytes!("../../../fonts/JetBrainsMono-ExtraBoldItalic.ttf"),
    },
    BundledFont {
        filename: "JetBrainsMono-ExtraLight.ttf",
        data: include_bytes!("../../../fonts/JetBrainsMono-ExtraLight.ttf"),
    },
    BundledFont {
        filename: "JetBrainsMono-ExtraLightItalic.ttf",
        data: include_bytes!("../../../fonts/JetBrainsMono-ExtraLightItalic.ttf"),
    },
    BundledFont {
        filename: "JetBrainsMono-Light.ttf",
        data: include_bytes!("../../../fonts/JetBrainsMono-Light.ttf"),
    },
    BundledFont {
        filename: "JetBrainsMono-LightItalic.ttf",
        data: include_bytes!("../../../fonts/JetBrainsMono-LightItalic.ttf"),
    },
    BundledFont {
        filename: "JetBrainsMono-MediumItalic.ttf",
        data: include_bytes!("../../../fonts/JetBrainsMono-MediumItalic.ttf"),
    },
];

/// Ensures that bundled fonts are written to the user font directory and registered
/// into the current process's font table without downloading any files.
pub fn ensure_bundled_fonts() {
    #[cfg(windows)]
    {
        // 1. Register font bytes directly into memory font table using Win32 GDI AddFontMemResourceEx
        register_in_memory_fonts();

        // 2. Ensure font files are placed in user fonts directory so fontdb/DirectWrite discovers them on disk
        sync_and_register_disk_fonts();
    }
}

#[cfg(windows)]
fn register_in_memory_fonts() {
    unsafe {
        extern "system" {
            fn AddFontMemResourceEx(
                pbFont: *const u8,
                cbFont: u32,
                pdv: *mut std::ffi::c_void,
                pcFonts: *mut u32,
            ) -> *mut std::ffi::c_void;
        }

        for font in BUNDLED_FONTS {
            let mut num_fonts: u32 = 0;
            let _ = AddFontMemResourceEx(
                font.data.as_ptr(),
                font.data.len() as u32,
                std::ptr::null_mut(),
                &mut num_fonts,
            );
        }
    }
}

#[cfg(windows)]
fn sync_and_register_disk_fonts() {
    use std::os::windows::ffi::OsStrExt;

    unsafe {
        extern "system" {
            fn AddFontResourceExW(
                name: *const u16,
                fl: u32,
                res: *mut std::ffi::c_void,
            ) -> i32;
        }

        const FR_PRIVATE: u32 = 0x10;

        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
        if local_app_data.is_empty() {
            return;
        }

        let user_fonts_dir = std::path::PathBuf::from(&local_app_data)
            .join("Microsoft")
            .join("Windows")
            .join("Fonts");

        let _ = std::fs::create_dir_all(&user_fonts_dir);

        for font in BUNDLED_FONTS {
            let target_path = user_fonts_dir.join(font.filename);
            let should_write = match std::fs::metadata(&target_path) {
                Ok(meta) => meta.len() != font.data.len() as u64,
                Err(_) => true,
            };

            if should_write {
                let _ = std::fs::write(&target_path, font.data);
            }

            let wide: Vec<u16> = target_path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let _ = AddFontResourceExW(wide.as_ptr(), FR_PRIVATE, std::ptr::null_mut());
        }

        // Also alias JetBrainsMono-Medium.ttf as JetBrainsMono-Regular.ttf if not present
        let regular_path = user_fonts_dir.join("JetBrainsMono-Regular.ttf");
        if !regular_path.exists() {
            if let Some(medium_font) = BUNDLED_FONTS.first() {
                let _ = std::fs::write(&regular_path, medium_font.data);
                let wide: Vec<u16> = regular_path
                    .as_os_str()
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                let _ = AddFontResourceExW(wide.as_ptr(), FR_PRIVATE, std::ptr::null_mut());
            }
        }
    }
}
