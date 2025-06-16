use std::sync::Arc;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use rosc::{OscMessage, OscPacket, OscType};
use chrono::Local;
use crate::types::{AppStateMutex, TimerManagerMutex, TimerEvent};
use crate::utils::{vrc_float_to_hour, vrc_float_to_minute};
use crate::timer::calculate_and_set_next_alarm;

// OSCサーバー
pub struct OscServer {
    state: AppStateMutex,
    timer_manager: TimerManagerMutex,
    client_socket: Arc<UdpSocket>,
}

impl OscServer {
    pub async fn new(state: AppStateMutex, timer_manager: TimerManagerMutex) -> Result<Self, Box<dyn std::error::Error>> {
        let client_socket = UdpSocket::bind("0.0.0.0:0").await?;
        Ok(Self { 
            state,
            timer_manager,
            client_socket: Arc::new(client_socket),
        })
    }

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

    async fn handle_osc_message(&self, msg: OscMessage) {
        let mut state = self.state.lock().unwrap();
        state.last_osc_received = Some(Local::now());
        
        println!("Received OSC message: {} with {} args", msg.addr, msg.args.len());
        
        match msg.addr.as_str() {
            "/avatar/parameters/AlarmSetHour" => {
                if let Some(OscType::Float(hour_float)) = msg.args.first() {
                    let hour = vrc_float_to_hour(*hour_float);
                    state.alarm_set_hour = *hour_float;
                    println!("  AlarmSetHour updated: {} ({}h)", hour_float, hour);
                    
                    drop(state);
                    let state_clone = self.state.clone();
                    let timer_mgr_clone = self.timer_manager.clone();
                    tokio::spawn(async move {
                        calculate_and_set_next_alarm(&state_clone, &timer_mgr_clone).await;
                    });
                    return;
                }
            }
            "/avatar/parameters/AlarmSetMinute" => {
                if let Some(OscType::Float(minute_float)) = msg.args.first() {
                    let minute = vrc_float_to_minute(*minute_float);
                    state.alarm_set_minute = *minute_float;
                    println!("  AlarmSetMinute updated: {} ({}m)", minute_float, minute);
                    
                    drop(state);
                    let state_clone = self.state.clone();
                    let timer_mgr_clone = self.timer_manager.clone();
                    tokio::spawn(async move {
                        calculate_and_set_next_alarm(&state_clone, &timer_mgr_clone).await;
                    });
                    return;
                }
            }
            "/avatar/parameters/AlarmIsOn" => {
                if let Some(OscType::Bool(is_on)) = msg.args.first() {
                    state.alarm_is_on = *is_on;
                    println!("  AlarmIsOn updated to: {}", is_on);
                    
                    drop(state);
                    let state_clone = self.state.clone();
                    let timer_mgr_clone = self.timer_manager.clone();
                    tokio::spawn(async move {
                        calculate_and_set_next_alarm(&state_clone, &timer_mgr_clone).await;
                    });
                    return;
                }
            }
            "/avatar/parameters/SnoozePressed" => {
                if let Some(OscType::Bool(pressed)) = msg.args.first() {
                    if *pressed && state.is_ringing {
                        state.snooze_pressed = *pressed;
                        println!("  Snooze button pressed");
                        
                        drop(state);
                        let state_clone = self.state.clone();
                        let timer_mgr_clone = self.timer_manager.clone();
                        tokio::spawn(async move {
                            handle_timer_event_sync(&state_clone, &timer_mgr_clone, TimerEvent::SnoozeEnd);
                        });
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
                        println!("  Stop button pressed");
                        
                        drop(state);
                        let state_clone = self.state.clone();
                        let timer_mgr_clone = self.timer_manager.clone();
                        tokio::spawn(async move {
                            handle_timer_event_sync(&state_clone, &timer_mgr_clone, TimerEvent::Stop);
                        });
                        return;
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

// OSC送信
pub async fn send_osc_to_vrchat(address: &str, args: Vec<OscType>, state: &AppStateMutex) -> Result<(), String> {
    let target_ip = "127.0.0.1";
    let target_port = 9000;
    
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

// 同期版のタイマーイベント処理（Sendエラー回避）
fn handle_timer_event_sync(state: &AppStateMutex, timer_manager: &TimerManagerMutex, event: TimerEvent) {
    use crate::timer::handle_timer_event;
    
    let state_clone = state.clone();
    let timer_mgr_clone = timer_manager.clone();
    tokio::spawn(async move {
        handle_timer_event(state_clone, timer_mgr_clone, event).await;
    });
}