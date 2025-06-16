use std::fs;
use std::path::PathBuf;
use crate::types::AlarmSettings;

// 設定ファイル管理
pub fn get_config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("vrc-osc-alarm");
    path.push("settings.json");
    path
}

pub fn load_settings() -> AlarmSettings {
    let config_path = get_config_path();
    
    if config_path.exists() {
        if let Ok(content) = fs::read_to_string(&config_path) {
            if let Ok(settings) = serde_json::from_str::<AlarmSettings>(&content) {
                println!("Loaded settings from: {:?}", config_path);
                return settings;
            }
        }
    }
    
    println!("Using default settings");
    AlarmSettings::default()
}

pub fn save_settings(settings: &AlarmSettings) -> Result<(), String> {
    let config_path = get_config_path();
    
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    
    fs::write(&config_path, content)
        .map_err(|e| format!("Failed to write settings file: {}", e))?;
    
    println!("Saved settings to: {:?}", config_path);
    Ok(())
}