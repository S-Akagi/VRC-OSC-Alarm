use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};
use rosc::OscType;

// モジュール定義
mod types;
mod utils;
mod config;
mod osc;
mod timer;
mod commands;

// 必要なモジュールのインポート
use types::{AppState, TimerManager};
use utils::{hour_to_vrc_float, minute_to_vrc_float};
use config::load_settings;
use osc::{OscServer, send_osc_to_vrchat};
use timer::calculate_and_set_next_alarm;
use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // アプリ状態とタイマー管理を初期化
    let initial_state = Arc::new(Mutex::new(AppState::default()));
    let timer_manager = Arc::new(Mutex::new(TimerManager::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(initial_state.clone())
        .manage(timer_manager.clone())
        .setup(move |app| {
            // 状態とタイマー管理のクローンを作成
            let state = initial_state.clone();
            let timer_mgr = timer_manager.clone();

            let _handle = app.handle().clone();

            // OSCサーバー用の状態クローン
            let server_state = state.clone();
            let server_timer_mgr = timer_mgr.clone();
            // OSCサーバーを非同期で起動
            tauri::async_runtime::spawn(async move {
                let osc_server = match OscServer::new(server_state, server_timer_mgr).await {
                    Ok(server) => server,
                    Err(e) => {
                        eprintln!("Failed to create OSC server: {}", e);
                        return;
                    }
                };

                if let Err(e) = osc_server.start(9001).await {
                    eprintln!("OSC Server error: {}", e);
                }
            });

            // 起動時処理用の状態クローン
            let startup_state = state.clone();
            let startup_timer_mgr = timer_mgr.clone();
            // 起動時の設定読み込みと送信を非同期で実行
            tauri::async_runtime::spawn(async move {
                // VRChatへの接続を待つための遅延
                sleep(Duration::from_secs(2)).await;

                let settings = load_settings();
                // VRChat形式に変換
                let hour_vrc = hour_to_vrc_float(settings.alarm_hour);
                let minute_vrc = minute_to_vrc_float(settings.alarm_minute);

                if let Err(e) = send_osc_to_vrchat("/avatar/parameters/AlarmSetHour", vec![OscType::Float(hour_vrc)], &startup_state).await {
                    eprintln!("Failed to send AlarmSetHour on startup: {}", e);
                }
                if let Err(e) = send_osc_to_vrchat("/avatar/parameters/AlarmSetMinute", vec![OscType::Float(minute_vrc)], &startup_state).await {
                    eprintln!("Failed to send AlarmSetMinute on startup: {}", e);
                }
                if let Err(e) = send_osc_to_vrchat("/avatar/parameters/AlarmIsOn", vec![OscType::Bool(settings.alarm_is_on)], &startup_state).await {
                    eprintln!("Failed to send AlarmIsOn on startup: {}", e);
                }

                println!("Sent saved settings to VRChat on startup: {}:{} (VRC: {:.3}, {:.3})",
                         settings.alarm_hour, settings.alarm_minute, hour_vrc, minute_vrc);

                // アプリ状態を初期化
                {
                    let mut app_state = startup_state.lock().unwrap();
                    app_state.alarm_set_hour = hour_vrc;
                    app_state.alarm_set_minute = minute_vrc;
                    app_state.alarm_is_on = settings.alarm_is_on;
                    app_state.snooze_count = 0;
                    app_state.max_snoozes = settings.max_snoozes;
                    app_state.ringing_duration_minutes = settings.ringing_duration_minutes;
                    app_state.snooze_duration_minutes = settings.snooze_duration_minutes;
                }
                
                // 次のアラームを計算してタイマーをセット
                calculate_and_set_next_alarm(startup_state, startup_timer_mgr).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_current_state,
            send_osc,
            send_alarm_should_fire,
            send_alarm_set_hour,
            send_alarm_set_minute,
            send_alarm_is_on,
            send_snooze_pressed,
            send_stop_pressed,
            load_and_send_settings,
            save_alarm_settings,
            get_alarm_settings,
            save_timer_settings,
            get_timer_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}