use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    // --- デバイス設定 ---
    pub input_device_name: String,
    pub output_device_1_name: String,
    pub output_device_2_name: String,
    pub monitor_device_name: String,

    // --- 有効/無効（ミュート）状態 ---
    pub input_enabled: bool,
    pub output_device_1_enabled: bool,
    pub output_device_2_enabled: bool,
    pub monitor_enabled: bool,

    // --- モニター音量 ---
    pub monitor_volume: f32,

    // --- 切り替えモード ---
    pub switching_mode: String,

    // --- ショートカットキー ---
    pub output_device_1_hotkey: String,
    pub output_device_2_hotkey: String,

    // --- スタートアップ ---
    pub auto_start: bool,

    // --- アイコン・ラベル色設定 ---
    pub icon_color_out1_on: [u8; 3],
    pub icon_color_out2_on: [u8; 3],
    pub icon_color_both_off: [u8; 3],

    // --- ウィンドウ位置・サイズ ---
    pub window_pos_x: Option<f32>,
    pub window_pos_y: Option<f32>,
    pub window_size_x: Option<f32>,
    pub window_size_y: Option<f32>,

    // --- トレイアイコンサイズ ---
    pub tray_icon_size: u32,

    // --- 音量メーター設定 ---
    pub meter_width: f32,
    pub meter_height: f32,
    pub meter_rms_scale: f32,
    pub meter_decay_speed: f32,
    pub meter_peak_hold_secs: f64,
    pub meter_peak_decay_speed: f32,
    pub meter_safe_zone: f32,
    pub meter_warn_zone: f32,
    pub meter_color_safe: [u8; 3],
    pub meter_color_warn: [u8; 3],
    pub meter_color_danger: [u8; 3],
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
            icon_color_out1_on: [255, 100, 100],
            icon_color_out2_on: [100, 255, 100],
            icon_color_both_off: [150, 150, 150],
            window_pos_x: None,
            window_pos_y: None,
            window_size_x: None,
            window_size_y: None,
            tray_icon_size: 32,
            meter_width: 150.0,
            meter_height: 16.0,
            meter_rms_scale: 5.0,
            meter_decay_speed: 1.0,
            meter_peak_hold_secs: 1.5,
            meter_peak_decay_speed: 0.5,
            meter_safe_zone: 0.7,
            meter_warn_zone: 0.9,
            meter_color_safe: [40, 200, 40],
            meter_color_warn: [220, 200, 40],
            meter_color_danger: [220, 40, 40],
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
