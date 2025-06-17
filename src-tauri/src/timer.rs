use crate::osc::send_osc_to_vrchat;
use crate::types::{AppStateMutex, TimerEvent, TimerManagerMutex};
use crate::utils::{vrc_float_to_hour, vrc_float_to_minute};
use chrono::{Local, Timelike};
use rosc::OscType;
use std::future::Future;
use std::pin::Pin;
use tokio::time::{sleep, Duration};

// 次のアラームの時刻を計算し、タイマーを設定する
pub fn calculate_and_set_next_alarm(
    state: AppStateMutex,
    timer_manager: TimerManagerMutex,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        // 現在動作中のタイマーをキャンセル
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

        // アラームの設定を取得
        let (alarm_on, alarm_hour, alarm_minute) = {
            let app_state = match state.lock() {
                Ok(state) => state,
                Err(e) => {
                    eprintln!("Failed to lock state: {}", e);
                    return;
                }
            };
            (
                app_state.alarm_is_on,
                app_state.alarm_set_hour,
                app_state.alarm_set_minute,
            )
        };

        // アラームがオフの場合は何もしない
        if !alarm_on {
            println!("Alarm is OFF, no timer set");
            return;
        }
        // 現在時刻を取得
        let now = match tokio::task::spawn_blocking(Local::now).await {
            Ok(now) => now,
            Err(e) => {
                eprintln!("Could not get local time from blocking thread: {}", e);
                return;
            }
        };

        // VRCの浮動小数点数値を時分に変換
        let alarm_hour = vrc_float_to_hour(alarm_hour) as u32;
        let alarm_minute = vrc_float_to_minute(alarm_minute) as u32;

        // アラームの目標時刻を作成（秒とナノ秒は0に設定）
        let mut target_time = now
            .with_hour(alarm_hour)
            .and_then(|t| t.with_minute(alarm_minute))
            .and_then(|t| t.with_second(0))
            .and_then(|t| t.with_nanosecond(0))
            .unwrap();

        // 目標時刻が現在時刻より過去の場合は翔日に設定
        if now >= target_time {
            target_time += chrono::Duration::days(1);
        }

        // アラームまでの待機時間を計算
        let wait_duration = target_time.signed_duration_since(now);
        let wait_std_duration = Duration::from_millis(wait_duration.num_milliseconds() as u64);

        // 次のアラーム時刻をログ出力
        println!(
            "Next alarm set for: {} (in {} minutes)",
            target_time.format("%Y-%m-%d %H:%M:%S"),
            wait_duration.num_minutes()
        );

        // アラーム発火用のタイマーを作成
        let state_clone = state.clone();
        let timer_manager_clone = timer_manager.clone();

        let timer_handle = tokio::spawn(async move {
            // 指定した時間だけ待機
            sleep(wait_std_duration).await;
            // スヌーズ回数をリセット
            if let Ok(mut app_state) = state_clone.lock() {
                app_state.snooze_count = 0;
            }
            // アラーム発火イベントを発生
            handle_timer_event(state_clone, timer_manager_clone, TimerEvent::AlarmFire).await;
        });
        // タイマーをアクティブに設定
        if let Ok(mut timer_mgr) = timer_manager.lock() {
            timer_mgr.set_active_timer(timer_handle);
        }
    })
}

// アラーム関連のイベントを処理するメイン関数
pub fn handle_timer_event(
    state: AppStateMutex,
    timer_manager: TimerManagerMutex,
    event: TimerEvent,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        match event {
            // アラーム発火時の処理
            TimerEvent::AlarmFire => {
                println!("Alarm firing!");
                // VRChatにアラーム発火シグナルを送信
                if let Err(e) = send_osc_to_vrchat(
                    "/avatar/parameters/AlarmShouldFire",
                    vec![OscType::Bool(true)],
                    &state,
                )
                .await
                {
                    eprintln!("Failed to send alarm signal: {}", e);
                }

                // アラームの状態を有効にし、アラーム時間を取得
                let ringing_duration = {
                    let mut app_state = state.lock().unwrap();
                    app_state.is_ringing = true;
                    app_state.ringing_duration_minutes
                };

                // アラーム終了用のタイマーを作成
                let state_clone = state.clone();
                let timer_manager_clone = timer_manager.clone();
                let ringing_handle = tokio::spawn(async move {
                    // 設定したアラーム時間だけ待機
                    sleep(Duration::from_secs(ringing_duration as u64 * 60)).await;
                    println!(
                        "{} minutes of ringing completed. Auto-triggering snooze.",
                        ringing_duration
                    );
                    // アラーム終了イベントを発生
                    handle_timer_event(state_clone, timer_manager_clone, TimerEvent::RingingEnd)
                        .await;
                });

                // アラーム終了タイマーをアクティブに設定
                if let Ok(mut timer_mgr) = timer_manager.lock() {
                    timer_mgr.set_active_timer(ringing_handle);
                }
            }
            // スヌーズ終了またはアラーム終了時の処理
            TimerEvent::SnoozeEnd | TimerEvent::RingingEnd => {
                // スヌーズ回数を管理し、停止判定を行う
                let (should_stop, snooze_duration) = {
                    let mut app_state = state.lock().unwrap();
                    if matches!(event, TimerEvent::SnoozeEnd) {
                        app_state.snooze_count += 1;
                        println!(
                            "Manual snooze triggered. Count: {}/{}",
                            app_state.snooze_count, app_state.max_snoozes
                        );
                    } else {
                        app_state.snooze_count += 1;
                        println!(
                            "Auto snooze triggered. Count: {}/{}",
                            app_state.snooze_count, app_state.max_snoozes
                        );
                    }
                    let should_stop = app_state.snooze_count > app_state.max_snoozes;
                    app_state.is_ringing = false; // アラームを停止
                    if should_stop {
                        app_state.snooze_count = 0; // カウンターをリセット
                        println!("Max snoozes reached. Stopping alarm completely.");
                    }
                    (should_stop, app_state.snooze_duration_minutes)
                };

                // 現在動作中のタイマーをキャンセル
                {
                    let mut timer_mgr = timer_manager.lock().unwrap();
                    timer_mgr.cancel_active_timer();
                }

                // アラーム停止シグナルを送信
                tokio::spawn({
                    let state_clone = state.clone();
                    async move {
                        if let Err(e) = send_osc_to_vrchat(
                            "/avatar/parameters/AlarmShouldFire",
                            vec![OscType::Bool(false)],
                            &state_clone,
                        )
                        .await
                        {
                            eprintln!("Failed to send alarm stop signal: {}", e);
                        } else {
                            println!("Successfully sent AlarmShouldFire false");
                        }
                    }
                });

                // 最大スヌーズ回数に達した場合の処理
                if should_stop {
                    // 最終停止シグナルをVRChatに送信
                    if let Err(e) = send_osc_to_vrchat(
                        "/avatar/parameters/AlarmShouldFire",
                        vec![OscType::Bool(false)],
                        &state,
                    )
                    .await
                    {
                        eprintln!("Failed to send final alarm stop signal: {}", e);
                    }
                    // 次のアラームを設定
                    calculate_and_set_next_alarm(state, timer_manager).await;
                    return;
                }

                // スヌーズ間隔終了後のアラーム再発火用タイマーを作成
                let state_clone = state.clone();
                let timer_manager_clone = timer_manager.clone();
                let snooze_handle = tokio::spawn(async move {
                    // スヌーズ間隔だけ待機
                    sleep(Duration::from_secs(snooze_duration as u64 * 60)).await;
                    println!(
                        "Snooze duration ({} minutes) completed. Re-firing alarm.",
                        snooze_duration
                    );
                    // アラームを再発火
                    handle_timer_event(state_clone, timer_manager_clone, TimerEvent::AlarmFire)
                        .await;
                });

                // スヌーズタイマーをアクティブに設定
                if let Ok(mut timer_mgr) = timer_manager.lock() {
                    timer_mgr.set_active_timer(snooze_handle);
                }
            }
            // 手動停止時の処理
            TimerEvent::Stop => {
                // タイマーとアラーム状態をリセット
                {
                    let mut timer_mgr = timer_manager.lock().unwrap();
                    timer_mgr.cancel_active_timer(); // タイマーをキャンセル
                    let mut app_state = state.lock().unwrap();
                    app_state.is_ringing = false; // アラームを停止
                    app_state.snooze_count = 0; // スヌーズ回数をリセット
                    println!("Alarm stopped completely.");
                }

                // VRChatに停止シグナルを送信
                if let Err(e) = send_osc_to_vrchat(
                    "/avatar/parameters/AlarmShouldFire",
                    vec![OscType::Bool(false)],
                    &state,
                )
                .await
                {
                    eprintln!("Failed to send alarm stop signal: {}", e);
                }
                // 次のアラームを設定
                calculate_and_set_next_alarm(state, timer_manager).await;
            }
        }
    })
}
