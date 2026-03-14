use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const SETTINGS_FOLDER_NAME: &str = "CrashLog Collector";
const SETTINGS_FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedSettings {
    pub log_dir: String,
    pub overwrite_plugins_dir: String,
    pub mo2_dir: String,
    pub output_dir: String,
    #[serde(default)]
    pub max_chunks: String,
}

pub fn load_settings() -> Result<Option<PersistedSettings>, String> {
    load_settings_from_path(&default_settings_path())
}

pub fn save_settings(settings: &PersistedSettings) -> Result<(), String> {
    save_settings_to_path(&default_settings_path(), settings)
}

fn default_settings_path() -> PathBuf {
    let base_dir = config_dir()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));

    base_dir.join(SETTINGS_FOLDER_NAME).join(SETTINGS_FILE_NAME)
}

fn load_settings_from_path(path: &Path) -> Result<Option<PersistedSettings>, String> {
    if !path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read '{}': {error}", path.display()))?;

    serde_json::from_str(&contents)
        .map(Some)
        .map_err(|error| format!("failed to parse '{}': {error}", path.display()))
}

fn save_settings_to_path(path: &Path, settings: &PersistedSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create '{}': {error}", parent.display()))?;
    }

    let json = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("failed to serialize settings: {error}"))?;

    fs::write(path, json).map_err(|error| format!("failed to write '{}': {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn missing_settings_file_returns_none() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("missing.json");

        assert_eq!(load_settings_from_path(&path).unwrap(), None);
    }

    #[test]
    fn settings_round_trip_through_json_file() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("settings.json");
        let settings = PersistedSettings {
            log_dir: String::from("C:\\Logs"),
            overwrite_plugins_dir: String::from("C:\\Overwrite\\SKSE\\Plugins"),
            mo2_dir: String::from("C:\\MO2\\Profile"),
            output_dir: String::from("C:\\Output"),
            max_chunks: String::from("12"),
        };

        save_settings_to_path(&path, &settings).unwrap();
        let loaded = load_settings_from_path(&path).unwrap();

        assert_eq!(loaded, Some(settings));
    }

    #[test]
    fn settings_backfill_missing_max_chunks() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("settings.json");
        let legacy = r#"{
  "log_dir": "C:\\Logs",
  "overwrite_plugins_dir": "",
  "mo2_dir": "C:\\MO2\\Profile",
  "output_dir": "C:\\Output"
}"#;

        fs::write(&path, legacy).unwrap();
        let loaded = load_settings_from_path(&path).unwrap().unwrap();

        assert_eq!(loaded.max_chunks, "");
    }
}
