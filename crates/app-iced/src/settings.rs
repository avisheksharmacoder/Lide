//! Persistent configuration storage for Lide IDE.
//!
//! Handles loading and saving settings to `~/.lide/settings.json`
//! with graceful fallback to defaults, atomic write capabilities,
//! and bidirectional synchronization with `LideApp` state.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{LideApp, ThemeMode};

fn default_ide_font_size() -> f32 {
    20.0
}

/// Persistent application configuration settings schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LideSettings {
    pub theme_name: String,
    pub theme_mode: String,
    pub editor_font_size: f32,
    #[serde(default = "default_ide_font_size")]
    pub ide_font_size: f32,
    pub line_height_px: f64,
    pub format_on_save_enabled: bool,
    pub line_numbers_enabled: bool,
    pub ai_endpoint: String,
    pub ai_model_name: String,
    pub editor_font: String,
}

impl Default for LideSettings {
    fn default() -> Self {
        Self {
            theme_name: "One Dark".to_string(),
            theme_mode: "Dark".to_string(),
            editor_font_size: 14.0,
            ide_font_size: 20.0,
            line_height_px: 22.0,
            format_on_save_enabled: true,
            line_numbers_enabled: true,
            ai_endpoint: "http://localhost:11434".to_string(),
            ai_model_name: "qwen2.5-coder:7b".to_string(),
            editor_font: "JetBrains Mono".to_string(),
        }
    }
}

impl LideSettings {
    /// Extracts a configuration snapshot from the current `LideApp` state.
    pub fn from_app(app: &LideApp) -> Self {
        Self {
            theme_name: app.active_theme_name.clone(),
            theme_mode: match app.theme_mode {
                ThemeMode::Dark => "Dark".to_string(),
                ThemeMode::Light => "Light".to_string(),
            },
            editor_font_size: app.editor_font_size,
            ide_font_size: app.ide_font_size,
            line_height_px: app.line_height_px,
            format_on_save_enabled: app.format_on_save_enabled,
            line_numbers_enabled: app.line_numbers_enabled,
            ai_endpoint: app.ai_endpoint.clone(),
            ai_model_name: app.ai_model_name.clone(),
            editor_font: app.editor_font.clone(),
        }
    }

    /// Resolves the default directory path for Lide configuration (`~/.lide`).
    pub fn config_dir() -> PathBuf {
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            PathBuf::from(profile).join(".lide")
        } else if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join(".lide")
        } else {
            PathBuf::from(".lide")
        }
    }

    /// Resolves the default path to the settings file (`~/.lide/settings.json`).
    pub fn default_settings_path() -> PathBuf {
        Self::config_dir().join("settings.json")
    }

    /// Loads settings from the default path, falling back to defaults if missing or corrupted.
    pub fn load() -> Self {
        let path = Self::default_settings_path();
        Self::load_from_path(&path).unwrap_or_default()
    }

    /// Loads settings from a specific file path.
    pub fn load_from_path(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    }

    /// Saves settings to the default path (`~/.lide/settings.json`).
    pub fn save(&self) -> Result<(), String> {
        Self::save_to_path(self, &Self::default_settings_path())
    }

    /// Saves settings to the specified file path, creating parent directories if needed.
    pub fn save_to_path(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = fs::create_dir_all(parent);
            }
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| format!("Serialization error: {e}"))?;
        fs::write(path, json).map_err(|e| format!("Cannot write settings file: {e}"))?;
        Ok(())
    }
}
