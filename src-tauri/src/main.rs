/*
 * VRChat Anywhere Alarm
 * Copyright (c) 2024 S-Akagi
 * 
 * This software incorporates components from various open source projects.
 * See LICENSE file for complete license information.
 * 
 * This software is provided "as is" without warranty of any kind.
 * VRChat is a trademark of VRChat Inc. This software is not affiliated with VRChat Inc.
 */

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    vrc_osc_alarm_lib::run();
}
