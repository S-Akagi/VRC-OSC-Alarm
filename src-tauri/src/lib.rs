use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;
use rosc::{OscMessage, OscPacket};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Local};

// OSC受信状態
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    pub last_osc_received: Option<DateTime<Local>>,
}

// デフォルト状態
impl Default for AppState {
    fn default() -> Self {
        Self {
            last_osc_received: None,
        }
    }
}

pub type AppStateMutex = Arc<Mutex<AppState>>;

// OSCサーバーの設定
struct OscServer {
    state: AppStateMutex,
}

// OSCサーバーの設定
impl OscServer {
    fn new(state: AppStateMutex) -> Self {
        Self { state }
    }

    // OSCサーバーの起動
    async fn start(&self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
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
        
        // メッセージの詳細をログに出力
        for (i, arg) in msg.args.iter().enumerate() {
            println!("  Arg {}: {:?}", i, arg);
        }
    }
}
#[tauri::command]
fn get_current_state(state: tauri::State<AppStateMutex>) -> Result<AppState, String> {
    match state.lock() {
        Ok(app_state) => Ok(app_state.clone()),
        Err(e) => Err(format!("Failed to get state: {}", e)),
    }
}


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_state = Arc::new(Mutex::new(AppState::default()));
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(initial_state.clone())
        .setup(move |app| {
            let state = initial_state.clone();
            let osc_server = OscServer::new(state);
            
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = osc_server.start(9001).await {
                    eprintln!("OSC Server error: {}", e);
                }
            });
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_current_state])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
