use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;
use rosc::{OscMessage, OscPacket, OscType};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Local, Timelike};
use std::net::SocketAddr;
use tokio::time::{sleep, Duration};
use std::fs;
use std::path::PathBuf;
use tokio::task::JoinHandle;

// OSC受信・送信状態
#[derive(Debug, Serialize, Deserialize)]
pub struct AppState {
    pub last_osc_received: Option<DateTime<Local>>,
    pub last_osc_sent: Option<DateTime<Local>>,
    pub alarm_set_hour: f32,
    pub alarm_set_minute: f32,
    pub alarm_is_on: bool,
    pub snooze_pressed: bool,
    pub stop_pressed: bool,
    pub is_ringing: bool,
    pub snooze_count: u32,
    pub max_snoozes: u32,
    #[serde(skip)]
    pub active_timer_handle: Option<JoinHandle<()>>,
    #[serde(skip)]
    pub ringing_timer_handle: Option<JoinHandle<()>>,
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            last_osc_received: self.last_osc_received,
            last_osc_sent: self.last_osc_sent,
            alarm_set_hour: self.alarm_set_hour,
            alarm_set_minute: self.alarm_set_minute,
            alarm_is_on: self.alarm_is_on,
            snooze_pressed: self.snooze_pressed,
            stop_pressed: self.stop_pressed,
            is_ringing: self.is_ringing,
            snooze_count: self.snooze_count,
            max_snoozes: self.max_snoozes,
            active_timer_handle: None, // JoinHandleはCloneできないのでNoneにする
            ringing_timer_handle: None,
        }
    }
}



// デフォルト状態
impl Default for AppState {
    fn default() -> Self {
        Self {
            last_osc_received: None,
            last_osc_sent: None,
            alarm_set_hour: 0.0,
            alarm_set_minute: 0.0,
            alarm_is_on: false,
            snooze_pressed: false,
            stop_pressed: false,
            is_ringing: false,
            snooze_count: 0,
            max_snoozes: 5, // デフォルトは5回
            active_timer_handle: None,
            ringing_timer_handle: None,
        }
    }
}

pub type AppStateMutex = Arc<Mutex<AppState>>;

// 設定データ構造（時間単位で保存）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmSettings {
    pub alarm_hour: i32,    // 0-23
    pub alarm_minute: i32,  // 0-59
    pub alarm_is_on: bool,
}

impl Default for AlarmSettings {
    fn default() -> Self {
        Self {
            alarm_hour: 7,
            alarm_minute: 0,
            alarm_is_on: false,
        }
    }
}

// 時間をVRC用のfloat値に変換（直感的な0.01=1時間形式）
fn hour_to_vrc_float(hour: i32) -> f32 {
    let clamped_hour = hour.clamp(0, 23);
    (clamped_hour as f32) / 100.0  // 0.01 = 1時間
}

fn minute_to_vrc_float(minute: i32) -> f32 {
    let clamped_minute = minute.clamp(0, 59);
    (clamped_minute as f32) / 100.0  // 0.01 = 1分
}

// VRC用のfloat値を時間に変換
fn vrc_float_to_hour(value: f32) -> i32 {
    let hour = (value * 100.0).round() as i32;
    hour.clamp(0, 23)  // 範囲外なら丸め込み
}

fn vrc_float_to_minute(value: f32) -> i32 {
    let minute = (value * 100.0).round() as i32;
    minute.clamp(0, 59)  // 範囲外なら丸め込み
}

// 設定ファイル管理
fn get_config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("vrc-osc-alarm");
    path.push("settings.json");
    path
}

fn load_settings() -> AlarmSettings {
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

fn save_settings(settings: &AlarmSettings) -> Result<(), String> {
    let config_path = get_config_path();
    
    // ディレクトリを作成
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

// OSCサーバーとクライアントの設定
struct OscServer {
    state: AppStateMutex,
    client_socket: Arc<UdpSocket>,
}

// OSCサーバーとクライアントの設定
impl OscServer {
    async fn new(state: AppStateMutex) -> Result<Self, Box<dyn std::error::Error>> {
        let client_socket = UdpSocket::bind("0.0.0.0:0").await?;
        Ok(Self { 
            state,
            client_socket: Arc::new(client_socket),
        })
    }

    // OSCサーバーの起動
    async fn start(&self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        let addr = format!("127.0.0.1:{}", port);
        let socket = UdpSocket::bind(&addr).await?;
        println!("OSC Server listening on {}", addr);

        let mut buf = [0u8; 1024];
        
        // OSCメッセージの受信
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

    // OSCパケットの処理
    fn handle_osc_packet(&self, packet: OscPacket) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
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

    // OSCメッセージの処理
    async fn handle_osc_message(&self, msg: OscMessage) {
        let mut state = self.state.lock().unwrap();
        state.last_osc_received = Some(Local::now());
        
        println!("Received OSC message: {} with {} args", msg.addr, msg.args.len());
        
        // VRChatアラームパラメータの処理
        match msg.addr.as_str() {
            "/avatar/parameters/AlarmSetHour" => {
                if let Some(OscType::Float(hour_float)) = msg.args.first() {
                    let hour = vrc_float_to_hour(*hour_float);
                    state.alarm_set_hour = *hour_float;
                    println!("  AlarmSetHour updated: {} ({}h)", hour_float, hour);
                    
                    // タイマー再計算
                    drop(state);
                    let state_clone = self.state.clone();
                    tokio::spawn(async move {
                        recalculate_and_set_timer(&state_clone).await;
                    });
                    return;
                }
            }
            "/avatar/parameters/AlarmSetMinute" => {
                if let Some(OscType::Float(minute_float)) = msg.args.first() {
                    let minute = vrc_float_to_minute(*minute_float);
                    state.alarm_set_minute = *minute_float;
                    println!("  AlarmSetMinute updated: {} ({}m)", minute_float, minute);
                    
                    // タイマー再計算
                    drop(state);
                    let state_clone = self.state.clone();
                    tokio::spawn(async move {
                        recalculate_and_set_timer(&state_clone).await;
                    });
                    return;
                }
            }
            "/avatar/parameters/AlarmIsOn" => {
                if let Some(OscType::Bool(is_on)) = msg.args.first() {
                    state.alarm_is_on = *is_on;
                    println!("  AlarmIsOn updated to: {}", is_on);
                    
                    // タイマー再計算
                    drop(state);
                    let state_clone = self.state.clone();
                    tokio::spawn(async move {
                        recalculate_and_set_timer(&state_clone).await;
                    });
                    return;
                }
            }
            "/avatar/parameters/SnoozePressed" => {
                if let Some(OscType::Bool(pressed)) = msg.args.first() {
                    if *pressed && state.is_ringing {
                        state.snooze_pressed = *pressed;
                        println!("  Snooze button pressed - triggering snooze");
                        
                        let state_clone = self.state.clone();
                        tokio::spawn(async move {
                            trigger_snooze(&state_clone).await;
                        });
                        
                        drop(state);
                        return;
                    } else {
                        state.snooze_pressed = *pressed;
                        println!("  SnoozePressed updated to: {}", pressed);
                    }
                }
            }
            "/avatar/parameters/StopPressed" => {
                if let Some(OscType::Bool(pressed)) = msg.args.first() {
                    if *pressed && state.is_ringing {
                        state.stop_pressed = *pressed;
                        println!("  Stop button pressed - stopping alarm completely");
                        
                        let state_clone = self.state.clone();
                        tokio::spawn(async move {
                            stop_alarm_completely(&state_clone).await;
                        });
                        
                        drop(state);
                        return;
                    } else {
                        state.stop_pressed = *pressed;
                        println!("  StopPressed updated to: {}", pressed);
                    }
                }
            }
            _ => {
                // その他のメッセージの詳細をログに出力
                for (i, arg) in msg.args.iter().enumerate() {
                    println!("  Arg {}: {:?}", i, arg);
                }
            }
        }
    }

    // OSCメッセージの送信
    async fn send_osc_message(&self, addr: &str, args: Vec<OscType>, target: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
        let msg = OscMessage {
            addr: addr.to_string(),
            args,
        };
        
        let packet = OscPacket::Message(msg);
        let msg_buf = rosc::encoder::encode(&packet)?;
        
        self.client_socket.send_to(&msg_buf, target).await?;
        
        let mut state = self.state.lock().unwrap();
        state.last_osc_sent = Some(Local::now());
        
        println!("Sent OSC message to {}: {}", target, addr);
        Ok(())
    }
}

#[tauri::command]
async fn send_osc(
    address: String,
    value: String,
    target_ip: String,
    target_port: u16,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    println!("Sending OSC message to {}:{}", target_ip, target_port);
    println!("Address: {}", address);
    println!("Value: {}", value);

    let target: SocketAddr = format!("{}:{}", target_ip, target_port)
        .parse()
        .map_err(|e| format!("Invalid target address: {}", e))?;
    
    let args = vec![OscType::String(value)];
    
    let client_socket = UdpSocket::bind("0.0.0.0:0").await
        .map_err(|e| format!("Failed to bind client socket: {}", e))?;
    
    let msg = OscMessage {
        addr: address.clone(),
        args,
    };
    
    let packet = OscPacket::Message(msg);
    let msg_buf = rosc::encoder::encode(&packet)
        .map_err(|e| format!("Failed to encode OSC message: {}", e))?;
    
    client_socket.send_to(&msg_buf, target).await
        .map_err(|e| format!("Failed to send OSC message: {}", e))?;
    
    let mut app_state = state.lock()
        .map_err(|e| format!("Failed to lock state: {}", e))?;
    app_state.last_osc_sent = Some(Local::now());
    
    println!("Sent OSC message to {}: {}", target, address);
    Ok(())
}

#[tauri::command]
fn get_current_state(state: tauri::State<AppStateMutex>) -> Result<AppState, String> {
    match state.lock() {
        Ok(app_state) => Ok(app_state.clone()),
        Err(e) => Err(format!("Failed to get state: {}", e)),
    }
}

async fn send_osc_to_vrchat(address: &str, args: Vec<OscType>, state: &AppStateMutex) -> Result<(), String> {
    let target_ip = "127.0.0.1";
    let target_port = 9000; // VRChatのデフォルトポート
    
    let target: SocketAddr = format!("{}:{}", target_ip, target_port)
        .parse()
        .map_err(|e| format!("Invalid target address: {}", e))?;
    
    let client_socket = UdpSocket::bind("0.0.0.0:0").await
        .map_err(|e| format!("Failed to bind client socket: {}", e))?;
    
    let msg = OscMessage {
        addr: address.to_string(),
        args,
    };
    
    let packet = OscPacket::Message(msg);
    let msg_buf = rosc::encoder::encode(&packet)
        .map_err(|e| format!("Failed to encode OSC message: {}", e))?;
    
    client_socket.send_to(&msg_buf, target).await
        .map_err(|e| format!("Failed to send OSC message: {}", e))?;
    
    let mut app_state = state.lock()
        .map_err(|e| format!("Failed to lock state: {}", e))?;
    app_state.last_osc_sent = Some(Local::now());
    
    println!("Sent OSC to VRChat: {} at {}", address, target);
    Ok(())
}

#[tauri::command]
async fn send_alarm_should_fire(
    should_fire: bool,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let args = vec![OscType::Bool(should_fire)];
    send_osc_to_vrchat("/avatar/parameters/AlarmShouldFire", args, &state).await
}

#[tauri::command]
async fn send_alarm_set_hour(
    hour: i32,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let hour = hour.clamp(0, 23);
    let vrc_value = hour_to_vrc_float(hour);
    let args = vec![OscType::Float(vrc_value)];
    println!("hour: {}", vrc_value);
    send_osc_to_vrchat("/avatar/parameters/AlarmSetHour", args, &state).await
}

#[tauri::command]
async fn send_alarm_set_minute(
    minute: i32,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let minute = minute.clamp(0, 59);
    let vrc_value = minute_to_vrc_float(minute);
    let args = vec![OscType::Float(vrc_value)];
    println!("minute: {}", vrc_value);
    send_osc_to_vrchat("/avatar/parameters/AlarmSetMinute", args, &state).await
}

#[tauri::command]
async fn send_alarm_is_on(
    is_on: bool,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let args = vec![OscType::Bool(is_on)];
    send_osc_to_vrchat("/avatar/parameters/AlarmIsOn", args, &state).await
}

#[tauri::command]
async fn send_snooze_pressed(
    pressed: bool,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let args = vec![OscType::Bool(pressed)];
    send_osc_to_vrchat("/avatar/parameters/SnoozePressed", args, &state).await
}

#[tauri::command]
async fn send_stop_pressed(
    pressed: bool,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let args = vec![OscType::Bool(pressed)];
    send_osc_to_vrchat("/avatar/parameters/StopPressed", args, &state).await
}


#[tauri::command]
async fn load_and_send_settings(state: tauri::State<'_, AppStateMutex>) -> Result<AlarmSettings, String> {
    let settings = load_settings();
    
    // 設定値をVRChatに送信（時間を0.00-1.00範囲に変換）
    let hour_vrc = hour_to_vrc_float(settings.alarm_hour);
    let minute_vrc = minute_to_vrc_float(settings.alarm_minute);
    
    send_osc_to_vrchat("/avatar/parameters/AlarmSetHour", vec![OscType::Float(hour_vrc)], &state).await?;
    send_osc_to_vrchat("/avatar/parameters/AlarmSetMinute", vec![OscType::Float(minute_vrc)], &state).await?;
    send_osc_to_vrchat("/avatar/parameters/AlarmIsOn", vec![OscType::Bool(settings.alarm_is_on)], &state).await?;
    
    println!("Sent saved settings to VRChat: {}:{} (VRC: {:.3}, {:.3})", 
             settings.alarm_hour, settings.alarm_minute, hour_vrc, minute_vrc);
    Ok(settings)
}

#[tauri::command]
async fn save_alarm_settings(
    alarm_hour: i32,
    alarm_minute: i32,
    alarm_is_on: bool,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let settings = AlarmSettings {
        alarm_hour: alarm_hour.clamp(0, 23),
        alarm_minute: alarm_minute.clamp(0, 59),
        alarm_is_on,
    };
    
    save_settings(&settings)?;
    
    // 設定保存と同時にVRChatに送信（時間を0.00-1.00範囲に変換）
    let hour_vrc = hour_to_vrc_float(settings.alarm_hour);
    let minute_vrc = minute_to_vrc_float(settings.alarm_minute);
    
    send_osc_to_vrchat("/avatar/parameters/AlarmSetHour", vec![OscType::Float(hour_vrc)], &state).await?;
    send_osc_to_vrchat("/avatar/parameters/AlarmSetMinute", vec![OscType::Float(minute_vrc)], &state).await?;
    send_osc_to_vrchat("/avatar/parameters/AlarmIsOn", vec![OscType::Bool(settings.alarm_is_on)], &state).await?;
    
    println!("Saved and sent settings to VRChat: {}:{} (VRC: {:.3}, {:.3})", 
             settings.alarm_hour, settings.alarm_minute, hour_vrc, minute_vrc);
    Ok(())
}

#[tauri::command]
fn get_alarm_settings() -> Result<AlarmSettings, String> {
    Ok(load_settings())
}

// タイマー計算・セット関数
async fn recalculate_and_set_timer(state: &AppStateMutex) {
    let mut app_state = match state.lock() {
        Ok(state) => state,
        Err(e) => {
            eprintln!("Failed to lock state: {}", e);
            return;
        }
    };
    
    // 既存タイマーのキャンセル
    if let Some(handle) = app_state.active_timer_handle.take() {
        handle.abort();
        println!("Previous timer cancelled");
    }
    
    // アラームOFFなら終了
    if !app_state.alarm_is_on {
        println!("Alarm is OFF, no timer set");
        return;
    }
    
    // 現在時刻の取得
    let now = Local::now();
    
    // 設定時刻の取得（VRC float値から変換）
    let alarm_hour = vrc_float_to_hour(app_state.alarm_set_hour) as u32;
    let alarm_minute = vrc_float_to_minute(app_state.alarm_set_minute) as u32;
    
    // 今日の目標時刻を計算
    let mut target_time = now
        .with_hour(alarm_hour)
        .and_then(|t| t.with_minute(alarm_minute))
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0))
        .unwrap();
    
    // 現在時刻が目標時刻を過ぎていたら、明日の同じ時刻に設定
    if now >= target_time {
        target_time = target_time + chrono::Duration::days(1);
    }
    
    // 待機時間の計算
    let wait_duration = target_time.signed_duration_since(now);
    let wait_std_duration = Duration::from_millis(wait_duration.num_milliseconds() as u64);
    
    println!(
        "Next alarm set for: {} (in {} minutes)", 
        target_time.format("%Y-%m-%d %H:%M:%S"), 
        wait_duration.num_minutes()
    );
    
    // 新しいタイマーをセット
    let state_clone = state.clone();
    let timer_handle = tokio::spawn(async move {
        sleep(wait_std_duration).await;
        
        // スヌーズ回数をリセットして新しいアラームサイクル開始
        if let Ok(mut app_state) = state_clone.lock() {
            app_state.snooze_count = 0;
        }
        
        // アラーム発火処理を呼び出す
        fire_alarm(&state_clone).await;
    });
    
    app_state.active_timer_handle = Some(timer_handle);
}

// スヌーズ処理関数 (trigger_snooze)
async fn trigger_snooze(state: &AppStateMutex) {
    let mut app_state = match state.lock() {
        Ok(state) => state,
        Err(e) => {
            eprintln!("Failed to lock state in trigger_snooze: {}", e);
            return;
        }
    };
    
    // スヌーズ回数を増やす
    app_state.snooze_count += 1;
    println!("Snooze triggered. Count: {}/{}", app_state.snooze_count, app_state.max_snoozes);
    
    // 最大スヌーズ回数を超えた場合は完全停止
    if app_state.snooze_count > app_state.max_snoozes {
        println!("Max snoozes reached. Stopping alarm completely.");
        
        // 全タイマーをキャンセル
        if let Some(handle) = app_state.active_timer_handle.take() {
            handle.abort();
        }
        if let Some(handle) = app_state.ringing_timer_handle.take() {
            handle.abort();
        }
        
        // 状態リセット
        app_state.is_ringing = false;
        app_state.snooze_count = 0;
        
        drop(app_state);
        
        // 音を止めて翌日のアラームをセット
        if let Err(e) = send_osc_to_vrchat(
            "/avatar/parameters/AlarmShouldFire", 
            vec![OscType::Bool(false)], 
            state
        ).await {
            eprintln!("Failed to send alarm stop signal: {}", e);
        }
        
        recalculate_and_set_timer(state).await;
        return;
    }
    
    // 鳴動タイマーをキャンセル
    if let Some(handle) = app_state.ringing_timer_handle.take() {
        handle.abort();
    }
    
    app_state.is_ringing = false;
    drop(app_state);
    
    // 音を止める
    if let Err(e) = send_osc_to_vrchat(
        "/avatar/parameters/AlarmShouldFire", 
        vec![OscType::Bool(false)], 
        state
    ).await {
        eprintln!("Failed to send alarm stop signal: {}", e);
    }
    
    // 9分後に再発火するタイマーをセット
    let state_clone = state.clone();
    let snooze_handle = tokio::spawn(async move {
        sleep(Duration::from_secs(9 * 60)).await; // 9分待機
        fire_alarm(&state_clone).await;
    });
    
    if let Ok(mut app_state) = state.lock() {
        if let Some(old_handle) = app_state.active_timer_handle.take() {
            old_handle.abort();
        }
        app_state.active_timer_handle = Some(snooze_handle);
    }
}

// アラーム発火共通処理
async fn fire_alarm(state: &AppStateMutex) {
    println!("Alarm firing!");
    
    // VRChatにアラーム発火信号を送信
    if let Err(e) = send_osc_to_vrchat(
        "/avatar/parameters/AlarmShouldFire", 
        vec![OscType::Bool(true)], 
        state
    ).await {
        eprintln!("Failed to send alarm signal: {}", e);
    }
    
    // 状態更新
    if let Ok(mut app_state) = state.lock() {
        app_state.is_ringing = true;
    }
    
    // 15分後の鳴動タイマーをセット
    let state_clone = state.clone();
    let ringing_handle = tokio::spawn(async move {
        sleep(Duration::from_secs(15 * 60)).await; // 15分待機
        println!("15 minutes of ringing completed. Auto-triggering snooze.");
        trigger_snooze(&state_clone).await;
    });
    
    if let Ok(mut app_state) = state.lock() {
        if let Some(old_handle) = app_state.ringing_timer_handle.take() {
            old_handle.abort();
        }
        app_state.ringing_timer_handle = Some(ringing_handle);
    }
}

// 完全停止処理
async fn stop_alarm_completely(state: &AppStateMutex) {
    if let Ok(mut app_state) = state.lock() {
        // 全タイマーをキャンセル
        if let Some(handle) = app_state.active_timer_handle.take() {
            handle.abort();
        }
        if let Some(handle) = app_state.ringing_timer_handle.take() {
            handle.abort();
        }
        
        // 状態リセット
        app_state.is_ringing = false;
        app_state.snooze_count = 0;
    }
    
    // 音を止める
    if let Err(e) = send_osc_to_vrchat(
        "/avatar/parameters/AlarmShouldFire", 
        vec![OscType::Bool(false)], 
        state
    ).await {
        eprintln!("Failed to send alarm stop signal: {}", e);
    }
    
    // 翌日のアラームをセット
    recalculate_and_set_timer(state).await;
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_state = Arc::new(Mutex::new(AppState::default())); 
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(initial_state.clone())
        .setup(move |app| {
            let state = initial_state.clone();
            
            let handle = app.handle().clone();
            
            // OSCサーバーの起動
            let server_state = state.clone();
            tauri::async_runtime::spawn(async move {
                let osc_server = match OscServer::new(server_state).await {
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
            
            // 起動時の設定読み込みと送信
            let startup_state = state.clone();
            tauri::async_runtime::spawn(async move {
                // 少し待機してからOSCサーバー起動後に設定を送信
                sleep(Duration::from_secs(2)).await;
                
                let settings = load_settings();
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
                
                // アプリ起動時にタイマーをセット
                {
                    let mut app_state = startup_state.lock().unwrap();
                    app_state.alarm_set_hour = hour_vrc;
                    app_state.alarm_set_minute = minute_vrc;
                    app_state.alarm_is_on = settings.alarm_is_on;
                    app_state.snooze_count = 0; // 起動時はスヌーズ回数をリセット
                }
                
                recalculate_and_set_timer(&startup_state).await;
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
            get_alarm_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
