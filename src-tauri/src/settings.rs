use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::{AppHandle, Emitter, Manager};

fn default_true() -> bool {
    true
}

fn default_scale() -> u16 {
    200
}

pub fn valid_scale(value: u16) -> bool {
    matches!(value, 100 | 150 | 200 | 300)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_true")]
    pub confirm_delete: bool,
    #[serde(default = "default_true")]
    pub sound_enabled: bool,
    #[serde(default = "default_scale")]
    pub scale_percent: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            confirm_delete: true,
            sound_enabled: true,
            scale_percent: default_scale(),
        }
    }
}

pub struct SettingsState(pub Mutex<Settings>);

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir.join("settings.json"))
}

pub fn load(app: &AppHandle) -> Settings {
    let mut settings = settings_path(app)
        .ok()
        .and_then(|path| read_settings(&path))
        .unwrap_or_default();
    if !valid_scale(settings.scale_percent) {
        settings.scale_percent = default_scale();
    }
    settings
}

fn read_settings(path: &Path) -> Option<Settings> {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

fn write_settings(path: &Path, settings: &Settings) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    fs::write(path, bytes).map_err(|error| error.to_string())
}

pub fn current(app: &AppHandle) -> Result<Settings, String> {
    app.state::<SettingsState>()
        .0
        .lock()
        .map(|settings| settings.clone())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<Settings, String> {
    current(&app)
}

pub fn toggle_confirm(app: &AppHandle) -> Result<Settings, String> {
    update(app, |settings| {
        settings.confirm_delete = !settings.confirm_delete
    })
}

pub fn toggle_sound(app: &AppHandle) -> Result<Settings, String> {
    update(app, |settings| {
        settings.sound_enabled = !settings.sound_enabled
    })
}

pub fn set_scale(app: &AppHandle, value: u16) -> Result<Settings, String> {
    if !valid_scale(value) {
        return Err("不支持的回收站尺寸".into());
    }
    update(app, |settings| settings.scale_percent = value)
}

fn update(app: &AppHandle, change: impl FnOnce(&mut Settings)) -> Result<Settings, String> {
    let state = app.state::<SettingsState>();
    let mut guard = state.0.lock().map_err(|error| error.to_string())?;
    let mut next = guard.clone();
    change(&mut next);
    write_settings(&settings_path(app)?, &next)?;
    *guard = next.clone();
    drop(guard);
    let _ = app.emit("bin://settings", next.clone());
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_have_safe_defaults() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert!(settings.confirm_delete);
        assert!(settings.sound_enabled);
        assert_eq!(settings.scale_percent, 200);
        assert_eq!(
            serde_json::to_value(settings).unwrap(),
            serde_json::json!({
                "confirmDelete": true,
                "soundEnabled": true,
                "scalePercent": 200
            })
        );
    }

    #[test]
    fn preferences_survive_a_file_round_trip() {
        let path = std::env::temp_dir().join(format!(
            "cat-recycle-bin-settings-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let modified = Settings {
            confirm_delete: false,
            sound_enabled: false,
            scale_percent: 150,
        };
        write_settings(&path, &modified).unwrap();
        let loaded = read_settings(&path).unwrap();
        let _ = fs::remove_file(&path);
        assert!(!loaded.confirm_delete);
        assert!(!loaded.sound_enabled);
        assert_eq!(loaded.scale_percent, 150);
    }
}
