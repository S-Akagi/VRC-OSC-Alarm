use crate::config::{load_settings, save_settings};
use crate::timer::{calculate_and_set_next_alarm, handle_timer_event};
use crate::types::{AlarmSettings, AppStateMutex, TimerEvent, TimerManagerMutex};
use crate::utils::{hour_to_vrc_float, minute_to_vrc_float, vrc_float_to_hour, vrc_float_to_minute};
use chrono::Utc;
use rosc::{OscMessage, OscPacket, OscType};
use std::net::SocketAddr;
use tauri::Emitter;
use tokio::net::UdpSocket;

/// OSCサーバー構造体
pub struct OscServer {
    state: AppStateMutex,
    timer_manager: TimerManagerMutex,
    app_handle: Option<tauri::AppHandle>,
}

impl OscServer {
    /// 新しいOSCサーバーを作成
    pub async fn new(
        state: AppStateMutex,
        timer_manager: TimerManagerMutex,
        app_handle: Option<tauri::AppHandle>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            state,
            timer_manager,
            app_handle,
        })
    }

    // OSCサーバーを起動
    pub async fn start(&self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        let addr = format!("127.0.0.1:{}", port);
        let socket = UdpSocket::bind(&addr).await?;
        println!("OSC Server listening on {}", addr);

        let mut buf = [0u8; 1024];

        loop {
            match socket.recv_from(&mut buf).await {
                Ok((size, _addr)) => {
                    if let Ok((_buf, packet)) = rosc::decoder::decode_udp(&buf[..size]) {
                        self.handle_osc_packet(packet).await;
                    }
                }
                Err(e) => {
                    eprintln!("Error receiving OSC message: {}", e);
                }
            }
        }
    }

    // OSCパケットを処理
    fn handle_osc_packet(
        &self,
        packet: OscPacket,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            match packet {
                OscPacket::Message(msg) => {
                    self.handle_osc_message(msg).await;
                }
                OscPacket::Bundle(bundle) => {
                    for packet in bundle.content {
                        self.handle_osc_packet(packet).await;
                    }
                }
            }
        })
    }

    // OSCメッセージを処理
    async fn handle_osc_message(&self, msg: OscMessage) {
        let mut state = self.state.lock().unwrap();
        state.last_osc_received = Some(Utc::now());

        println!(
            "Received OSC message: {} with {} args",
            msg.addr,
            msg.args.len()
        );

        // OSCメッセージのアドレスに応じて処理
        match msg.addr.as_str() {
            "/avatar/parameters/AlarmSetHour" => {
                // アラーム時間を設定
                if let Some(OscType::Float(hour_float)) = msg.args.first() {
                    let hour = vrc_float_to_hour(*hour_float);
                    let clamped_vrc_value = hour_to_vrc_float(hour);
                    state.alarm_set_hour = clamped_vrc_value;
                    println!("  AlarmSetHour updated: {} -> {} ({}h)", hour_float, clamped_vrc_value, hour);

                    // 値が変更された場合のみVRC側に再送信
                    if (*hour_float - clamped_vrc_value).abs() > 0.001 {
                        let state_clone = self.state.clone();
                        tokio::spawn(async move {
                            if let Err(e) = send_osc_to_vrchat(
                                "/avatar/parameters/AlarmSetHour",
                                vec![OscType::Float(clamped_vrc_value)],
                                &state_clone,
                            ).await {
                                eprintln!("Failed to sync AlarmSetHour to VRC: {}", e);
                            }
                        });
                    }

                    // 設定を保存
                    let current_settings = load_settings();
                    let new_settings = AlarmSettings {
                        alarm_hour: hour,
                        alarm_minute: current_settings.alarm_minute,
                        alarm_is_on: current_settings.alarm_is_on,
                        max_snoozes: current_settings.max_snoozes,
                        ringing_duration_minutes: current_settings.ringing_duration_minutes,
                        snooze_duration_minutes: current_settings.snooze_duration_minutes,
                    };
                    if let Err(e) = save_settings(&new_settings) {
                        eprintln!("Failed to save hour setting: {}", e);
                    }

                    // UIに設定変更を通知
                    if let Some(ref handle) = self.app_handle {
                        if let Err(e) = handle.emit("alarm-settings-changed", &new_settings) {
                            eprintln!("Failed to emit alarm settings changed event: {}", e);
                        }
                    }

                    drop(state);
                    let state_clone = self.state.clone();
                    let timer_mgr_clone = self.timer_manager.clone();
                    tokio::spawn(calculate_and_set_next_alarm(state_clone, timer_mgr_clone));
                }
            }
            "/avatar/parameters/AlarmSetMinute" => {
                // アラーム分を設定
                if let Some(OscType::Float(minute_float)) = msg.args.first() {
                    let minute = vrc_float_to_minute(*minute_float);
                    let clamped_vrc_value = minute_to_vrc_float(minute);
                    state.alarm_set_minute = clamped_vrc_value;
                    println!("  AlarmSetMinute updated: {} -> {} ({}m)", minute_float, clamped_vrc_value, minute);

                    // 値が変更された場合のみVRC側に再送信
                    if (*minute_float - clamped_vrc_value).abs() > 0.001 {
                        let state_clone = self.state.clone();
                        tokio::spawn(async move {
                            if let Err(e) = send_osc_to_vrchat(
                                "/avatar/parameters/AlarmSetMinute",
                                vec![OscType::Float(clamped_vrc_value)],
                                &state_clone,
                            ).await {
                                eprintln!("Failed to sync AlarmSetMinute to VRC: {}", e);
                            }
                        });
                    }

                    // 設定を保存
                    let current_settings = load_settings();
                    let new_settings = AlarmSettings {
                        alarm_hour: current_settings.alarm_hour,
                        alarm_minute: minute,
                        alarm_is_on: current_settings.alarm_is_on,
                        max_snoozes: current_settings.max_snoozes,
                        ringing_duration_minutes: current_settings.ringing_duration_minutes,
                        snooze_duration_minutes: current_settings.snooze_duration_minutes,
                    };
                    if let Err(e) = save_settings(&new_settings) {
                        eprintln!("Failed to save minute setting: {}", e);
                    }

                    // UIに設定変更を通知
                    if let Some(ref handle) = self.app_handle {
                        if let Err(e) = handle.emit("alarm-settings-changed", &new_settings) {
                            eprintln!("Failed to emit alarm settings changed event: {}", e);
                        }
                    }

                    drop(state);
                    let state_clone = self.state.clone();
                    let timer_mgr_clone = self.timer_manager.clone();
                    tokio::spawn(calculate_and_set_next_alarm(state_clone, timer_mgr_clone));
                }
            }
            "/avatar/parameters/AlarmIsOn" => {
                // アラームがオンかどうか
                if let Some(OscType::Bool(is_on)) = msg.args.first() {
                    state.alarm_is_on = *is_on;
                    println!("  AlarmIsOn updated to: {}", is_on);

                    // 設定を保存
                    let current_settings = load_settings();
                    let new_settings = AlarmSettings {
                        alarm_hour: current_settings.alarm_hour,
                        alarm_minute: current_settings.alarm_minute,
                        alarm_is_on: *is_on,
                        max_snoozes: current_settings.max_snoozes,
                        ringing_duration_minutes: current_settings.ringing_duration_minutes,
                        snooze_duration_minutes: current_settings.snooze_duration_minutes,
                    };
                    if let Err(e) = save_settings(&new_settings) {
                        eprintln!("Failed to save alarm_is_on setting: {}", e);
                    }

                    // UIに設定変更を通知
                    if let Some(ref handle) = self.app_handle {
                        if let Err(e) = handle.emit("alarm-settings-changed", &new_settings) {
                            eprintln!("Failed to emit alarm settings changed event: {}", e);
                        }
                    }

                    drop(state);
                    let state_clone = self.state.clone();
                    let timer_mgr_clone = self.timer_manager.clone();
                    tokio::spawn(calculate_and_set_next_alarm(state_clone, timer_mgr_clone));
                }
            }
            "/avatar/parameters/SnoozePressed" => {
                // スヌーズボタンが押されたかどうか
                if let Some(OscType::Bool(pressed)) = msg.args.first() {
                    if *pressed && state.is_ringing {
                        state.snooze_pressed = *pressed;
                        println!("  Snooze button pressed");

                        drop(state);
                        let state_clone = self.state.clone();
                        let timer_mgr_clone = self.timer_manager.clone();
                        handle_timer_event_sync(
                            state_clone,
                            timer_mgr_clone,
                            TimerEvent::SnoozeEnd,
                        );
                    } else {
                        state.snooze_pressed = *pressed;
                        println!("  SnoozePressed updated to: {}", pressed);
                    }
                }
            }
            "/avatar/parameters/StopPressed" => {
                // ストップボタンが押されたかどうか
                if let Some(OscType::Bool(pressed)) = msg.args.first() {
                    if *pressed && state.is_ringing {
                        state.stop_pressed = *pressed;
                        println!("  Stop button pressed");

                        drop(state);
                        let state_clone = self.state.clone();
                        let timer_mgr_clone = self.timer_manager.clone();
                        handle_timer_event_sync(state_clone, timer_mgr_clone, TimerEvent::Stop);
                    } else {
                        state.stop_pressed = *pressed;
                        println!("  StopPressed updated to: {}", pressed);
                    }
                }
            }
            _ => {
                for (i, arg) in msg.args.iter().enumerate() {
                    println!("  Arg {}: {:?}", i, arg);
                }
            }
        }
    }
}

// OSCメッセージをVRChatに送信
pub async fn send_osc_to_vrchat(
    address: &str,
    args: Vec<OscType>,
    state: &AppStateMutex,
) -> Result<(), String> {
    let target_ip = "127.0.0.1";
    let target_port = 9000;

    let target: SocketAddr = format!("{}:{}", target_ip, target_port)
        .parse()
        .map_err(|e| format!("Invalid target address: {}", e))?;

    let client_socket = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| format!("Failed to bind client socket: {}", e))?;

    let msg = OscMessage {
        addr: address.to_string(),
        args,
    };

    let packet = OscPacket::Message(msg);
    let msg_buf = rosc::encoder::encode(&packet)
        .map_err(|e| format!("Failed to encode OSC message: {}", e))?;

    client_socket
        .send_to(&msg_buf, target)
        .await
        .map_err(|e| format!("Failed to send OSC message: {}", e))?;

    // メッセージが送信されるのを待つ
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let mut app_state = state
        .lock()
        .map_err(|e| format!("Failed to lock state: {}", e))?;
    app_state.last_osc_sent = Some(Utc::now());

    println!("Sent OSC to VRChat: {} at {}", address, target);
    Ok(())
}

// タイマーイベントを処理
fn handle_timer_event_sync(
    state: AppStateMutex,
    timer_manager: TimerManagerMutex,
    event: TimerEvent,
) {
    tokio::spawn(handle_timer_event(state, timer_manager, event));
}
