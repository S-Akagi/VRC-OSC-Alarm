/*
 * Anywhere Alarm System
 * Copyright (c) 2024 S-Akagi
 * 
 * This software incorporates components from various open source projects.
 * See LICENSE file for complete license information.
 * 
 * This software is provided "as is" without warranty of any kind.
 * VRChat is a trademark of VRChat Inc. This software is not affiliated with VRChat Inc.
 */

use rosc::OscType;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

// モジュール定義
mod commands;
mod config;
mod osc;
#[cfg(feature = "pro")]
mod timer;
mod types;
mod utils;

// 必要なモジュールのインポート
use commands::*;
use config::load_settings;
use osc::{send_osc_to_vrchat, OscServer};
#[cfg(feature = "pro")]
use timer::calculate_and_set_next_alarm;
use types::{AppState, TimerManager};
use utils::{hour_to_vrc_float, minute_to_vrc_float};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // アプリ状態とタイマー管理を初期化
    let initial_state = Arc::new(Mutex::new(AppState::default()));
    let timer_manager = Arc::new(Mutex::new(TimerManager::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            let server_handle = _handle.clone();
            // OSCサーバーを非同期で起動
            tauri::async_runtime::spawn(async move {
                let osc_server = match OscServer::new(server_state, server_timer_mgr, Some(server_handle)).await {
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

            #[cfg(feature = "pro")]
            {
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

                    if let Err(e) = send_osc_to_vrchat(
                        "/avatar/parameters/AlarmSetHour",
                        vec![OscType::Float(hour_vrc)],
                        &startup_state,
                    )
                    .await
                    {
                        eprintln!("Failed to send AlarmSetHour on startup: {}", e);
                    }
                    if let Err(e) = send_osc_to_vrchat(
                        "/avatar/parameters/AlarmSetMinute",
                        vec![OscType::Float(minute_vrc)],
                        &startup_state,
                    )
                    .await
                    {
                        eprintln!("Failed to send AlarmSetMinute on startup: {}", e);
                    }
                    if let Err(e) = send_osc_to_vrchat(
                        "/avatar/parameters/AlarmIsOn",
                        vec![OscType::Bool(settings.alarm_is_on)],
                        &startup_state,
                    )
                    .await
                    {
                        eprintln!("Failed to send AlarmIsOn on startup: {}", e);
                    }


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
            }

            // ハートビート送信用の状態クローン
            let heartbeat_state = state.clone();
            // VRChatへのハートビート送信を開始
            tauri::async_runtime::spawn(async move {
                // 起動完了を待つ
                sleep(Duration::from_secs(5)).await;
                
                let mut interval = tokio::time::interval(Duration::from_secs(30)); // 30秒間隔
                loop {
                    interval.tick().await;
                    
                    // 現在の設定を取得してハートビートとして送信
                    let settings = load_settings();
                    
                    // ハートビートとして設定値をまとめて送信
                    if let Err(e) = osc::send_heartbeat_to_vrchat(&heartbeat_state, &settings).await {
                        eprintln!("Heartbeat failed: {}", e);
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // --- 無料版のコマンド ---
            get_current_state,
            get_current_version,
            check_for_updates,
            // --- Pro版のコマンド ---
            #[cfg(feature = "pro")]
            send_osc,
            #[cfg(feature = "pro")]
            send_alarm_should_fire,
            #[cfg(feature = "pro")]
            send_alarm_set_hour,
            #[cfg(feature = "pro")]
            send_alarm_set_minute,
            #[cfg(feature = "pro")]
            send_alarm_is_on,
            #[cfg(feature = "pro")]
            send_snooze_pressed,
            #[cfg(feature = "pro")]
            send_stop_pressed,
            #[cfg(feature = "pro")]
            load_and_send_settings,
            #[cfg(feature = "pro")]
            save_alarm_settings,
            #[cfg(feature = "pro")]
            get_alarm_settings,
            #[cfg(feature = "pro")]
            save_timer_settings,
            #[cfg(feature = "pro")]
            get_timer_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
