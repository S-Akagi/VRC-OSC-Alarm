use crate::config::{load_settings, save_settings};
use crate::osc::send_osc_to_vrchat;
use crate::types::{AlarmSettings, AppState, AppStateMutex};
use crate::utils::{hour_to_vrc_float, minute_to_vrc_float};
use chrono::Utc;
use rosc::{OscMessage, OscPacket, OscType};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::UdpSocket;

#[derive(Serialize, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: String,
    html_url: String,
    published_at: String,
}

#[derive(Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub download_url: String,
}

// OSC送信コマンド
#[tauri::command]
pub async fn send_osc(
    address: String,
    value: String,
    target_ip: String,
    target_port: u16,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let target: SocketAddr = format!("{}:{}", target_ip, target_port)
        .parse()
        .map_err(|e| format!("Invalid target address: {}", e))?;

    let client_socket = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| format!("Failed to bind client socket: {}", e))?;

    let msg = OscMessage {
        addr: address.clone(),
        args: vec![OscType::String(value)],
    };

    let packet = OscPacket::Message(msg);
    let msg_buf = rosc::encoder::encode(&packet)
        .map_err(|e| format!("Failed to encode OSC message: {}", e))?;

    client_socket
        .send_to(&msg_buf, target)
        .await
        .map_err(|e| format!("Failed to send OSC message: {}", e))?;

    state
        .lock()
        .map_err(|e| format!("Failed to lock state: {}", e))?
        .last_osc_sent = Some(Utc::now());

    Ok(())
}

// 現在のアプリ状態を取得
#[tauri::command]
pub fn get_current_state(state: tauri::State<AppStateMutex>) -> Result<AppState, String> {
    match state.lock() {
        Ok(app_state) => Ok(app_state.clone()),
        Err(e) => Err(format!("Failed to get state: {}", e)),
    }
}

// アラームが鳴るかどうかをVRChatに送信
#[tauri::command]
pub async fn send_alarm_should_fire(
    should_fire: bool,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let args = vec![OscType::Bool(should_fire)];
    send_osc_to_vrchat("/avatar/parameters/AlarmShouldFire", args, &state).await
}

// アラーム時間をVRChatに送信
#[tauri::command]
pub async fn send_alarm_set_hour(
    hour: i32,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let hour = hour.clamp(0, 23);
    let vrc_value = hour_to_vrc_float(hour);
    let args = vec![OscType::Float(vrc_value)];
    send_osc_to_vrchat("/avatar/parameters/AlarmSetHour", args, &state).await
}

// アラーム分をVRChatに送信
#[tauri::command]
pub async fn send_alarm_set_minute(
    minute: i32,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    // 分を0-59の範囲に丸め込み
    let minute = minute.clamp(0, 59);
    let vrc_value = minute_to_vrc_float(minute);
    let args = vec![OscType::Float(vrc_value)];
    send_osc_to_vrchat("/avatar/parameters/AlarmSetMinute", args, &state).await
}

// アラーム有効フラグをVRChatに送信
#[tauri::command]
pub async fn send_alarm_is_on(
    is_on: bool,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let args = vec![OscType::Bool(is_on)];
    send_osc_to_vrchat("/avatar/parameters/AlarmIsOn", args, &state).await
}

// スヌーズボタンの状態をVRChatに送信
#[tauri::command]
pub async fn send_snooze_pressed(
    pressed: bool,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let args = vec![OscType::Bool(pressed)];
    send_osc_to_vrchat("/avatar/parameters/SnoozePressed", args, &state).await
}

// ストップボタンの状態をVRChatに送信
#[tauri::command]
pub async fn send_stop_pressed(
    pressed: bool,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let args = vec![OscType::Bool(pressed)];
    send_osc_to_vrchat("/avatar/parameters/StopPressed", args, &state).await
}

// 保存されたアラーム設定を読み込み、VRChatに送信
#[tauri::command]
pub async fn load_and_send_settings(
    state: tauri::State<'_, AppStateMutex>,
) -> Result<AlarmSettings, String> {
    let settings = load_settings();

    let hour_vrc = hour_to_vrc_float(settings.alarm_hour);
    let minute_vrc = minute_to_vrc_float(settings.alarm_minute);

    send_osc_to_vrchat(
        "/avatar/parameters/AlarmSetHour",
        vec![OscType::Float(hour_vrc)],
        &state,
    )
    .await?;
    send_osc_to_vrchat(
        "/avatar/parameters/AlarmSetMinute",
        vec![OscType::Float(minute_vrc)],
        &state,
    )
    .await?;
    send_osc_to_vrchat(
        "/avatar/parameters/AlarmIsOn",
        vec![OscType::Bool(settings.alarm_is_on)],
        &state,
    )
    .await?;

    println!(
        "Sent saved settings to VRChat: {}:{} (VRC: {:.3}, {:.3})",
        settings.alarm_hour, settings.alarm_minute, hour_vrc, minute_vrc
    );
    Ok(settings)
}

// アラーム設定を保存し、VRChatに送信
#[tauri::command]
pub async fn save_alarm_settings(
    alarm_hour: i32,
    alarm_minute: i32,
    alarm_is_on: bool,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    // 現在の設定を取得し、アラーム設定を更新
    let current_settings = load_settings();
    let settings = AlarmSettings {
        // 時を有効範囲に丸め込み
        alarm_hour: alarm_hour.clamp(0, 23),
        // 分を有効範囲に丸め込み
        alarm_minute: alarm_minute.clamp(0, 59),
        alarm_is_on,
        max_snoozes: current_settings.max_snoozes,
        ringing_duration_minutes: current_settings.ringing_duration_minutes,
        snooze_duration_minutes: current_settings.snooze_duration_minutes,
    };

    save_settings(&settings)?;

    // VRChat形式に変換して送信
    let hour_vrc = hour_to_vrc_float(settings.alarm_hour);
    let minute_vrc = minute_to_vrc_float(settings.alarm_minute);

    send_osc_to_vrchat(
        // アラーム時間をVRChatに送信
        "/avatar/parameters/AlarmSetHour",
        vec![OscType::Float(hour_vrc)],
        &state,
    )
    .await?;
    send_osc_to_vrchat(
        // アラーム分をVRChatに送信
        "/avatar/parameters/AlarmSetMinute",
        vec![OscType::Float(minute_vrc)],
        &state,
    )
    .await?;
    send_osc_to_vrchat(
        // アラーム有効フラグをVRChatに送信
        "/avatar/parameters/AlarmIsOn",
        vec![OscType::Bool(settings.alarm_is_on)],
        &state,
    )
    .await?;

    println!(
        "Saved and sent settings to VRChat: {}:{} (VRC: {:.3}, {:.3})",
        settings.alarm_hour, settings.alarm_minute, hour_vrc, minute_vrc
    );
    Ok(())
}

// アラーム設定を取得
#[tauri::command]
pub fn get_alarm_settings() -> Result<AlarmSettings, String> {
    Ok(load_settings())
}

// タイマー設定を保存
#[tauri::command]
pub async fn save_timer_settings(
    max_snoozes: u32,
    ringing_duration_minutes: u32,
    snooze_duration_minutes: u32,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    // 現在の設定を取得し、タイマー設定を更新
    let current_settings = load_settings();
    let settings = AlarmSettings {
        alarm_hour: current_settings.alarm_hour,
        alarm_minute: current_settings.alarm_minute,
        alarm_is_on: current_settings.alarm_is_on,
        // 各設定を有効範囲に丸め込み
        max_snoozes: max_snoozes.clamp(1, 20),
        ringing_duration_minutes: ringing_duration_minutes.clamp(1, 60),
        snooze_duration_minutes: snooze_duration_minutes.clamp(1, 30),
    };

    save_settings(&settings)?;

    // アプリ状態を更新
    {
        let mut app_state = state
            .lock()
            .map_err(|e| format!("Failed to lock state: {}", e))?;
        app_state.max_snoozes = settings.max_snoozes;
        app_state.ringing_duration_minutes = settings.ringing_duration_minutes;
        app_state.snooze_duration_minutes = settings.snooze_duration_minutes;
    }

    println!(
        "Saved timer settings: max_snoozes={}, ringing={}min, snooze={}min",
        settings.max_snoozes, settings.ringing_duration_minutes, settings.snooze_duration_minutes
    );
    Ok(())
}

// タイマー設定を取得
#[tauri::command]
pub fn get_timer_settings(state: tauri::State<AppStateMutex>) -> Result<(u32, u32, u32), String> {
    let app_state = state
        .lock()
        .map_err(|e| format!("Failed to lock state: {}", e))?;
    Ok((
        app_state.max_snoozes,
        app_state.ringing_duration_minutes,
        app_state.snooze_duration_minutes,
    ))
}

// 現在のバージョンを取得
#[tauri::command]
pub fn get_current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// アップデート確認
#[tauri::command]
pub async fn check_for_updates() -> Result<UpdateInfo, String> {
    let current_version = get_current_version();
    
    // GitHub Releases APIから最新バージョンを取得
    let url = "https://api.github.com/repos/S-Akagi/VRC-OSC-Alarm/releases/latest";
     let client = reqwest::Client::new();
    
    let response = client
        .get(url)
        .header("User-Agent", "VRC-OSC-Alarm")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch release info: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("GitHub API returned status: {}", response.status()));
    }
    
    let release: GitHubRelease = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse release info: {}", e))?;
    
    let latest_version = release.tag_name.trim_start_matches('v');
    let has_update = compare_versions(&current_version, latest_version);
    
    Ok(UpdateInfo {
        current_version,
        latest_version: latest_version.to_string(),
        has_update,
        download_url: release.html_url,
    })
}

// バージョン比較（簡易実装）
fn compare_versions(current: &str, latest: &str) -> bool {
    let current_parts: Vec<u32> = current
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    let latest_parts: Vec<u32> = latest
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    
    let max_len = current_parts.len().max(latest_parts.len());
    
    for i in 0..max_len {
        let current_part = current_parts.get(i).unwrap_or(&0);
        let latest_part = latest_parts.get(i).unwrap_or(&0);
        
        if latest_part > current_part {
            return true;
        } else if latest_part < current_part {
            return false;
        }
    }
    
    false
}
