use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

/// アプリケーションの状態を管理する構造体
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppState {
    pub last_osc_received: Option<DateTime<Utc>>,
    pub last_osc_sent: Option<DateTime<Utc>>,
    pub alarm_set_hour: f32,
    pub alarm_set_minute: f32,
    pub alarm_is_on: bool,
    pub snooze_pressed: bool,
    pub stop_pressed: bool,
    pub is_ringing: bool,
    pub snooze_count: u32,
    pub max_snoozes: u32,
    pub ringing_duration_minutes: u32,
    pub snooze_duration_minutes: u32,
}

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
            max_snoozes: 5,
            ringing_duration_minutes: 15,
            snooze_duration_minutes: 9,
        }
    }
}

pub type AppStateMutex = Arc<Mutex<AppState>>;

pub struct TimerManager {
    pub active_timer_handle: Option<JoinHandle<()>>,
}

impl TimerManager {
    pub fn new() -> Self {
        Self {
            active_timer_handle: None,
        }
    }

    pub fn cancel_active_timer(&mut self) {
        if let Some(handle) = self.active_timer_handle.take() {
            handle.abort();
            println!("Timer cancelled");
        }
    }

    pub fn set_active_timer(&mut self, handle: JoinHandle<()>) {
        self.cancel_active_timer();
        self.active_timer_handle = Some(handle);
    }
}

pub type TimerManagerMutex = Arc<Mutex<TimerManager>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmSettings {
    pub alarm_hour: i32,
    pub alarm_minute: i32,
    pub alarm_is_on: bool,
    pub max_snoozes: u32,
    pub ringing_duration_minutes: u32,
    pub snooze_duration_minutes: u32,
}

impl Default for AlarmSettings {
    fn default() -> Self {
        Self {
            alarm_hour: 7,
            alarm_minute: 0,
            alarm_is_on: false,
            max_snoozes: 5,
            ringing_duration_minutes: 15,
            snooze_duration_minutes: 9,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TimerEvent {
    AlarmFire,
    SnoozeEnd,
    RingingEnd,
    Stop,
}