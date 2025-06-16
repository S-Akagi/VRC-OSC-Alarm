use std::future::Future;
use std::pin::Pin;
use tokio::time::{sleep, Duration};
use chrono::{Local, Timelike};
use rosc::OscType;
use crate::types::{AppStateMutex, TimerManagerMutex, TimerEvent};
use crate::utils::{vrc_float_to_hour, vrc_float_to_minute};
use crate::osc::send_osc_to_vrchat;

/// 次のアラームを計算し、タイマーをセットする関数
pub fn calculate_and_set_next_alarm(
    state: AppStateMutex,
    timer_manager: TimerManagerMutex,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        {
            let mut timer_mgr = match timer_manager.lock() {
                Ok(mgr) => mgr,
                Err(e) => {
                    eprintln!("Failed to lock timer manager: {}", e);
                    return;
                }
            };
            timer_mgr.cancel_active_timer();
        }

        let (alarm_on, alarm_hour, alarm_minute) = {
            let app_state = match state.lock() {
                Ok(state) => state,
                Err(e) => {
                    eprintln!("Failed to lock state: {}", e);
                    return;
                }
            };
            (app_state.alarm_is_on, app_state.alarm_set_hour, app_state.alarm_set_minute)
        };

        if !alarm_on {
            println!("Alarm is OFF, no timer set");
            return;
        }

        let now = match tokio::task::spawn_blocking(Local::now).await {
            Ok(now) => now,
            Err(e) => {
                eprintln!("Could not get local time from blocking thread: {}", e);
                return;
            }
        };

        let alarm_hour = vrc_float_to_hour(alarm_hour) as u32;
        let alarm_minute = vrc_float_to_minute(alarm_minute) as u32;

        let mut target_time = now
            .with_hour(alarm_hour)
            .and_then(|t| t.with_minute(alarm_minute))
            .and_then(|t| t.with_second(0))
            .and_then(|t| t.with_nanosecond(0))
            .unwrap();

        if now >= target_time {
            target_time += chrono::Duration::days(1);
        }

        let wait_duration = target_time.signed_duration_since(now);
        let wait_std_duration = Duration::from_millis(wait_duration.num_milliseconds() as u64);

        println!(
            "Next alarm set for: {} (in {} minutes)",
            target_time.format("%Y-%m-%d %H:%M:%S"),
            wait_duration.num_minutes()
        );

        let state_clone = state.clone();
        let timer_manager_clone = timer_manager.clone();

        let timer_handle = tokio::spawn(async move {
            sleep(wait_std_duration).await;
            if let Ok(mut app_state) = state_clone.lock() {
                app_state.snooze_count = 0;
            }
            // Use the cloned values
            handle_timer_event(state_clone, timer_manager_clone, TimerEvent::AlarmFire).await;
        });
        if let Ok(mut timer_mgr) = timer_manager.lock() {
            timer_mgr.set_active_timer(timer_handle);
        }
    })
}

pub fn handle_timer_event(
    state: AppStateMutex,
    timer_manager: TimerManagerMutex,
    event: TimerEvent,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        match event {
            TimerEvent::AlarmFire => {
                println!("Alarm firing!");
                if let Err(e) = send_osc_to_vrchat(
                    "/avatar/parameters/AlarmShouldFire",
                    vec![OscType::Bool(true)],
                    &state,
                ).await {
                    eprintln!("Failed to send alarm signal: {}", e);
                }

                let ringing_duration = {
                    let mut app_state = state.lock().unwrap();
                    app_state.is_ringing = true;
                    app_state.ringing_duration_minutes
                };

                let state_clone = state.clone();
                let timer_manager_clone = timer_manager.clone();
                let ringing_handle = tokio::spawn(async move {
                    sleep(Duration::from_secs(ringing_duration as u64 * 60)).await;
                    println!("{} minutes of ringing completed. Auto-triggering snooze.", ringing_duration);
                    handle_timer_event(state_clone, timer_manager_clone, TimerEvent::RingingEnd).await;
                });

                if let Ok(mut timer_mgr) = timer_manager.lock() {
                    timer_mgr.set_active_timer(ringing_handle);
                }
            }
            TimerEvent::SnoozeEnd | TimerEvent::RingingEnd => {
                let (should_stop, snooze_duration) = {
                    let mut app_state = state.lock().unwrap();
                    if matches!(event, TimerEvent::SnoozeEnd) {
                        app_state.snooze_count += 1;
                        println!("Manual snooze triggered. Count: {}/{}", app_state.snooze_count, app_state.max_snoozes);
                    } else {
                        app_state.snooze_count += 1;
                        println!("Auto snooze triggered. Count: {}/{}", app_state.snooze_count, app_state.max_snoozes);
                    }
                    let should_stop = app_state.snooze_count > app_state.max_snoozes;
                    app_state.is_ringing = false;
                    if should_stop {
                        app_state.snooze_count = 0;
                        println!("Max snoozes reached. Stopping alarm completely.");
                    }
                    (should_stop, app_state.snooze_duration_minutes)
                };

                {
                    let mut timer_mgr = timer_manager.lock().unwrap();
                    timer_mgr.cancel_active_timer();
                }

                if let Err(e) = send_osc_to_vrchat(
                    "/avatar/parameters/AlarmShouldFire",
                    vec![OscType::Bool(false)],
                    &state,
                ).await {
                    eprintln!("Failed to send alarm stop signal: {}", e);
                }

                if should_stop {
                    calculate_and_set_next_alarm(state, timer_manager).await;
                    return;
                }

                let state_clone = state.clone();
                let timer_manager_clone = timer_manager.clone();
                let snooze_handle = tokio::spawn(async move {
                    sleep(Duration::from_secs(snooze_duration as u64 * 60)).await;
                    println!("Snooze duration ({} minutes) completed. Re-firing alarm.", snooze_duration);
                    handle_timer_event(state_clone, timer_manager_clone, TimerEvent::AlarmFire).await;
                });

                if let Ok(mut timer_mgr) = timer_manager.lock() {
                    timer_mgr.set_active_timer(snooze_handle);
                }
            }
            TimerEvent::Stop => {
                {
                    let mut timer_mgr = timer_manager.lock().unwrap();
                    timer_mgr.cancel_active_timer();
                    let mut app_state = state.lock().unwrap();
                    app_state.is_ringing = false;
                    app_state.snooze_count = 0;
                    println!("Alarm stopped completely.");
                }

                if let Err(e) = send_osc_to_vrchat(
                    "/avatar/parameters/AlarmShouldFire",
                    vec![OscType::Bool(false)],
                    &state,
                ).await {
                    eprintln!("Failed to send alarm stop signal: {}", e);
                }
                calculate_and_set_next_alarm(state, timer_manager).await;
            }
        }
    })
}