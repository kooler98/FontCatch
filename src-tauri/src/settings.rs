use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri_plugin_store::StoreExt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Settings {
    pub mode: Mode,
    pub font_preference: FontPreference,
    pub trash_after_install: bool,
    pub language: Language,
    pub folders: Vec<PathBuf>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Ask,
    Auto,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FontPreference {
    Variable,
    Static,
    Both,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Fr,
    En,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::Auto,
            font_preference: FontPreference::Variable,
            trash_after_install: false,
            language: Language::En,
            folders: dirs::download_dir().into_iter().collect(),
        }
    }
}
impl Settings {
    pub fn validate(&mut self) -> Result<(), String> {
        if self.folders.len() > 32 {
            return Err("At most 32 watched folders are supported.".into());
        }
        // Existing folders may be temporarily disconnected. Require an absolute path,
        // but do not silently discard a user's external drive configuration.
        if self.folders.iter().any(|p| !p.is_absolute()) {
            return Err("Folder paths must be absolute.".into());
        }
        let mut unique = Vec::new();
        for path in &self.folders {
            let path = path.canonicalize().unwrap_or_else(|_| path.clone());
            if !unique.contains(&path) {
                unique.push(path);
            }
        }
        self.folders = unique;
        Ok(())
    }
}
pub fn read(app: &tauri::AppHandle) -> Result<Settings, String> {
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let mut settings: Settings = match store.get("settings") {
        Some(value) => serde_json::from_value(value).map_err(|e| e.to_string())?,
        None => Settings::default(),
    };
    settings.validate()?;
    Ok(settings)
}
pub fn write(app: &tauri::AppHandle, mut settings: Settings) -> Result<Settings, String> {
    settings.validate()?;
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    let previous = store.get("settings");
    store.set(
        "settings",
        serde_json::to_value(&settings).map_err(|e| e.to_string())?,
    );
    if let Err(error) = store.save() {
        match previous {
            Some(value) => store.set("settings", value),
            None => {
                store.delete("settings");
            }
        }
        return Err(error.to_string());
    }
    Ok(settings)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn automatic_defaults_preserve_downloads() {
        let settings = Settings::default();
        assert_eq!(settings.mode, Mode::Auto);
        assert_eq!(settings.language, Language::En);
        assert_eq!(settings.font_preference, FontPreference::Variable);
        assert!(!settings.trash_after_install);
    }
    #[test]
    fn rejects_unknown_modes() {
        assert!(serde_json::from_str::<Settings>(r#"{"mode":"silently-delete"}"#).is_err());
    }
    #[test]
    fn rejects_relative_folders() {
        let mut settings = Settings {
            folders: vec![PathBuf::from("../Downloads")],
            ..Settings::default()
        };
        assert!(settings.validate().is_err());
    }
    #[test]
    fn deduplicates_folders() {
        let dir = std::env::temp_dir();
        let mut settings = Settings {
            folders: vec![dir.clone(), dir],
            ..Settings::default()
        };
        settings.validate().unwrap();
        assert_eq!(settings.folders.len(), 1);
    }
    #[test]
    fn settings_roundtrip() {
        let original = Settings::default();
        assert_eq!(
            original,
            serde_json::from_str(&serde_json::to_string(&original).unwrap()).unwrap()
        );
    }
}
