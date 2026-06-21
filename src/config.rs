use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub input_device_name: String,
    pub output_device_1_name: String,
    pub output_device_2_name: String,
    pub monitor_device_name: String,
    pub input_enabled: bool,
    pub output_device_1_enabled: bool,
    pub output_device_2_enabled: bool,
    pub monitor_enabled: bool,
    pub monitor_volume: f32,
    pub switching_mode: String,
    pub output_device_1_hotkey: String,
    pub output_device_2_hotkey: String,
    pub auto_start: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            input_device_name: "MIC (BRIDGE CAST V2)".to_string(),
            output_device_1_name: "CABLE-C Input".to_string(),
            output_device_2_name: "CABLE-D Input".to_string(),
            monitor_device_name: "GAME (BRIDGE CAST V2)".to_string(),
            input_enabled: true,
            output_device_1_enabled: true,
            output_device_2_enabled: false,
            monitor_enabled: true,
            monitor_volume: 0.8,
            switching_mode: "toggle".to_string(),
            output_device_1_hotkey: "Ctrl+Alt+Win+F9".to_string(),
            output_device_2_hotkey: "Ctrl+Alt+Win+F10".to_string(),
            auto_start: false,
        }
    }
}

pub fn load_config<P: AsRef<Path>>(path: P) -> Config {
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(config) = serde_json::from_str(&content) {
            return config;
        }
    }
    
    let default_config = Config::default();
    save_config(&path, &default_config);
    default_config
}

pub fn save_config<P: AsRef<Path>>(path: P, config: &Config) {
    if let Ok(content) = serde_json::to_string_pretty(config) {
        let _ = fs::write(path, content);
    }
}
