use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;
use rosc::{OscMessage, OscPacket, OscType};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Local};
use std::net::SocketAddr;

// OSC受信・送信状態
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    pub last_osc_received: Option<DateTime<Local>>,
    pub last_osc_sent: Option<DateTime<Local>>,
    pub alarm_set_hour: f32,
    pub alarm_set_minute: f32,
    pub alarm_is_on: bool,
    pub snooze_pressed: bool,
    pub stop_pressed: bool,
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
        }
    }
}

pub type AppStateMutex = Arc<Mutex<AppState>>;

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
                if let Some(OscType::Float(hour)) = msg.args.first() {
                    state.alarm_set_hour = *hour;
                    println!("  AlarmSetHour updated to: {}", hour);
                }
            }
            "/avatar/parameters/AlarmSetMinute" => {
                if let Some(OscType::Float(minute)) = msg.args.first() {
                    state.alarm_set_minute = *minute;
                    println!("  AlarmSetMinute updated to: {}", minute);
                }
            }
            "/avatar/parameters/AlarmIsOn" => {
                if let Some(OscType::Bool(is_on)) = msg.args.first() {
                    state.alarm_is_on = *is_on;
                    println!("  AlarmIsOn updated to: {}", is_on);
                }
            }
            "/avatar/parameters/SnoozePressed" => {
                if let Some(OscType::Bool(pressed)) = msg.args.first() {
                    state.snooze_pressed = *pressed;
                    println!("  SnoozePressed updated to: {}", pressed);
                }
            }
            "/avatar/parameters/StopPressed" => {
                if let Some(OscType::Bool(pressed)) = msg.args.first() {
                    state.stop_pressed = *pressed;
                    println!("  StopPressed updated to: {}", pressed);
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
    hour: f32,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let mut args = vec![];
    // 0.0から0.23の範囲に収める
    if hour < 0.0 {
        args.push(OscType::Float(0.0));
    } else if hour > 0.23 {
        args.push(OscType::Float(0.23));
    } else {
        args.push(OscType::Float(hour));
    }
    send_osc_to_vrchat("/avatar/parameters/AlarmSetHour", args, &state).await
}

#[tauri::command]
async fn send_alarm_set_minute(
    minute: f32,
    state: tauri::State<'_, AppStateMutex>,
) -> Result<(), String> {
    let mut args = vec![];
    // 0.0から0.59の範囲に収める
    if minute < 0.0 {
        args.push(OscType::Float(0.0));
    } else if minute > 0.59 {
        args.push(OscType::Float(0.59));
    } else {
        args.push(OscType::Float(minute));
    }
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_state = Arc::new(Mutex::new(AppState::default()));
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(initial_state.clone())
        .setup(move |app| {
            let state = initial_state.clone();
            
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let osc_server = match OscServer::new(state).await {
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
            send_stop_pressed
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
