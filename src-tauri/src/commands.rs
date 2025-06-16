use std::net::SocketAddr;
use tokio::net::UdpSocket;
use rosc::{OscMessage, OscPacket, OscType};
use chrono::Utc;
use crate::types::{AppState, AppStateMutex, AlarmSettings};
use crate::utils::{hour_to_vrc_float, minute_to_vrc_float};
use crate::osc::send_osc_to_vrchat;
use crate::config::{load_settings, save_settings};

/// OSCメッセージを送信するコマンド
#[tauri::command]
pub async fn send_osc(
    address: String,
    value: String,
    target_ip: String,
    target_port: u16,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    // OSC送信情報をログ出力
    println!("Sending OSC message to {}:{}", target_ip, target_port);
    println!("Address: {}", address);
    println!("Value: {}", value);

    // 送信先アドレスをパース
    let target: SocketAddr = format!("{}:{}", target_ip, target_port)
        .parse()
        .map_err(|e| format!("Invalid target address: {}", e))?;

    let args = vec![OscType::String(value)];

    // クライアントソケットを作成
    let client_socket = UdpSocket::bind("0.0.0.0:0").await
        .map_err(|e| format!("Failed to bind client socket: {}", e))?;

    // OSCメッセージを作成
    let msg = OscMessage {
        addr: address.clone(),
        args,
    };

    // メッセージをパケット化してエンコード
    let packet = OscPacket::Message(msg);
    let msg_buf = rosc::encoder::encode(&packet)
        .map_err(|e| format!("Failed to encode OSC message: {}", e))?;

    // メッセージを送信
    client_socket.send_to(&msg_buf, target).await
        .map_err(|e| format!("Failed to send OSC message: {}", e))?;

    // アプリ状態を更新
    let mut app_state = state.lock()
        .map_err(|e| format!("Failed to lock state: {}", e))?;
    app_state.last_osc_sent = Some(Utc::now());

    println!("Sent OSC message to {}: {}", target, address);
    Ok(())
}

/// アプリケーションの現在の状態を取得するコマンド
#[tauri::command]
pub fn get_current_state(state: tauri::State<AppStateMutex>) -> Result<AppState, String> {
    match state.lock() {
        Ok(app_state) => Ok(app_state.clone()),
        Err(e) => Err(format!("Failed to get state: {}", e)),
    }
}

/// アラーム発火フラグをVRChatに送信するコマンド
#[tauri::command]
pub async fn send_alarm_should_fire(
    should_fire: bool,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let args = vec![OscType::Bool(should_fire)];
    send_osc_to_vrchat("/avatar/parameters/AlarmShouldFire", args, &state).await
}

/// アラーム時をVRChatに送信するコマンド
#[tauri::command]
pub async fn send_alarm_set_hour(
    hour: i32,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    // 時を0-23の範囲に丸め込み
    let hour = hour.clamp(0, 23);
    let vrc_value = hour_to_vrc_float(hour);
    let args = vec![OscType::Float(vrc_value)];
    send_osc_to_vrchat("/avatar/parameters/AlarmSetHour", args, &state).await
}

/// アラーム分をVRChatに送信するコマンド
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

/// アラーム有効フラグをVRChatに送信するコマンド
#[tauri::command]
pub async fn send_alarm_is_on(
    is_on: bool,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let args = vec![OscType::Bool(is_on)];
    send_osc_to_vrchat("/avatar/parameters/AlarmIsOn", args, &state).await
}

/// スヌーズボタンの状態をVRChatに送信するコマンド
#[tauri::command]
pub async fn send_snooze_pressed(
    pressed: bool,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let args = vec![OscType::Bool(pressed)];
    send_osc_to_vrchat("/avatar/parameters/SnoozePressed", args, &state).await
}

/// ストップボタンの状態をVRChatに送信するコマンド
#[tauri::command]
pub async fn send_stop_pressed(
    pressed: bool,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let args = vec![OscType::Bool(pressed)];
    send_osc_to_vrchat("/avatar/parameters/StopPressed", args, &state).await
}

/// 保存されたアラーム設定を読み込み、VRChatに送信するコマンド
#[tauri::command]
pub async fn load_and_send_settings(state: tauri::State<'_, AppStateMutex>) -> Result<AlarmSettings, String> {
    let settings = load_settings();

    // VRChat形式に変換
    let hour_vrc = hour_to_vrc_float(settings.alarm_hour);
    let minute_vrc = minute_to_vrc_float(settings.alarm_minute);

    send_osc_to_vrchat("/avatar/parameters/AlarmSetHour", vec![OscType::Float(hour_vrc)], &state).await?;
    send_osc_to_vrchat("/avatar/parameters/AlarmSetMinute", vec![OscType::Float(minute_vrc)], &state).await?;
    send_osc_to_vrchat("/avatar/parameters/AlarmIsOn", vec![OscType::Bool(settings.alarm_is_on)], &state).await?;

    println!("Sent saved settings to VRChat: {}:{} (VRC: {:.3}, {:.3})",
             settings.alarm_hour, settings.alarm_minute, hour_vrc, minute_vrc);
    Ok(settings)
}

/// アラーム設定を保存し、VRChatに送信するコマンド
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

    send_osc_to_vrchat("/avatar/parameters/AlarmSetHour", vec![OscType::Float(hour_vrc)], &state).await?;
    send_osc_to_vrchat("/avatar/parameters/AlarmSetMinute", vec![OscType::Float(minute_vrc)], &state).await?;
    send_osc_to_vrchat("/avatar/parameters/AlarmIsOn", vec![OscType::Bool(settings.alarm_is_on)], &state).await?;

    println!("Saved and sent settings to VRChat: {}:{} (VRC: {:.3}, {:.3})",
             settings.alarm_hour, settings.alarm_minute, hour_vrc, minute_vrc);
    Ok(())
}

/// アラーム設定を取得するコマンド
#[tauri::command]
pub fn get_alarm_settings() -> Result<AlarmSettings, String> {
    Ok(load_settings())
}

/// タイマー設定を保存するコマンド
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
        let mut app_state = state.lock().map_err(|e| format!("Failed to lock state: {}", e))?;
        app_state.max_snoozes = settings.max_snoozes;
        app_state.ringing_duration_minutes = settings.ringing_duration_minutes;
        app_state.snooze_duration_minutes = settings.snooze_duration_minutes;
    }

    println!("Saved timer settings: max_snoozes={}, ringing={}min, snooze={}min",
             settings.max_snoozes, settings.ringing_duration_minutes, settings.snooze_duration_minutes);
    Ok(())
}

/// タイマー設定を取得するコマンド
#[tauri::command]
pub fn get_timer_settings(state: tauri::State<AppStateMutex>) -> Result<(u32, u32, u32), String> {
    let app_state = state.lock().map_err(|e| format!("Failed to lock state: {}", e))?;
    Ok((
        app_state.max_snoozes,
        app_state.ringing_duration_minutes,
        app_state.snooze_duration_minutes,
    ))
}