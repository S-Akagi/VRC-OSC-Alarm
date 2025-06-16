// ユーティリティ関数

pub fn hour_to_vrc_float(hour: i32) -> f32 {
    let clamped_hour = hour.clamp(0, 23);
    (clamped_hour as f32) / 100.0
}

pub fn minute_to_vrc_float(minute: i32) -> f32 {
    let clamped_minute = minute.clamp(0, 59);
    (clamped_minute as f32) / 100.0
}

pub fn vrc_float_to_hour(value: f32) -> i32 {
    let hour = (value * 100.0).round() as i32;
    hour.clamp(0, 23)
}

pub fn vrc_float_to_minute(value: f32) -> i32 {
    let minute = (value * 100.0).round() as i32;
    minute.clamp(0, 59)
}