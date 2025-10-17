use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Settings {
    pub volume: f32,
    pub speed: f64,
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 1.0,
            speed: 1.0,
            theme: "default".to_string(),
        }
    }
}

fn get_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|mut path| {
        path.push("bad-apple-player");
        path.push("config.json");
        path
    })
}

pub fn load_settings() -> Settings {
    if let Some(path) = get_config_path() {
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(settings) = serde_json::from_str(&data) {
                return settings;
            }
        }
    }
    Settings::default()
}

pub fn save_settings(settings: &Settings) {
    if let Some(path) = get_config_path() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
        }
        if let Ok(data) = serde_json::to_string_pretty(settings) {
            fs::write(path, data).ok();
        }
    }
}