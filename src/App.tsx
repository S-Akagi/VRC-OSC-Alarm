import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { event } from "@tauri-apps/api";

interface AppState {
	last_osc_received: string | null;
	last_osc_sent: string | null;
	alarm_set_hour: number;
	alarm_set_minute: number;
	alarm_is_on: boolean;
	snooze_pressed: boolean;
	stop_pressed: boolean;
}

interface AlarmSettings {
	alarm_hour: number;
	alarm_minute: number;
	alarm_is_on: boolean;
}

function App() {
	const [sendOscMsg, setSendOscMsg] = useState("");
	const [timerHour, setTimerHour] = useState(0);
	const [timerMinute, setTimerMinute] = useState(0);
	const [appState, setAppState] = useState<AppState | null>(null);
	const [savedSettings, setSavedSettings] = useState<AlarmSettings | null>(null);
	const [alarmIsOn, setAlarmIsOn] = useState(false);

	async function sendOsc() {
		try {
			await invoke("send_osc", {
				address: "/test/message",
				value: "Hello OSC",
				targetIp: "127.0.0.1",
				targetPort: 9000,
			});
			setSendOscMsg("OSC message sent successfully!");
		} catch (error) {
			setSendOscMsg(`Error: ${error}`);
		}
	}

	async function fetchAppState() {
		try {
			const state = await invoke<AppState>("get_current_state");
			setAppState(state);
		} catch (error) {
			console.error("Failed to fetch app state:", error);
		}
	}

	async function sendAlarmShouldFire(shouldFire: boolean) {
		try {
			await invoke("send_alarm_should_fire", { shouldFire });
			console.log(`Sent AlarmShouldFire: ${shouldFire}`);
		} catch (error) {
			console.error("Failed to send AlarmShouldFire:", error);
		}
	}

	async function sendAlarmSetHour(hour: number) {
		try {
			await invoke("send_alarm_set_hour", { hour });
			console.log(`Sent AlarmSetHour: ${hour}`);
		} catch (error) {
			console.error("Failed to send AlarmSetHour:", error);
		}
	}

	async function sendAlarmSetMinute(minute: number) {
		try {
			await invoke("send_alarm_set_minute", { minute });
			console.log(`Sent AlarmSetMinute: ${minute}`);
		} catch (error) {
			console.error("Failed to send AlarmSetMinute:", error);
		}
	}

	async function sendAlarmIsOn(isOn: boolean) {
		try {
			await invoke("send_alarm_is_on", { isOn });
			console.log(`Sent AlarmIsOn: ${isOn}`);
		} catch (error) {
			console.error("Failed to send AlarmIsOn:", error);
		}
	}

	async function sendSnoozePressed(pressed: boolean) {
		try {
			await invoke("send_snooze_pressed", { pressed });
			console.log(`Sent SnoozePressed: ${pressed}`);
		} catch (error) {
			console.error("Failed to send SnoozePressed:", error);
		}
	}

	async function sendStopPressed(pressed: boolean) {
		try {
			await invoke("send_stop_pressed", { pressed });
			console.log(`Sent StopPressed: ${pressed}`);
		} catch (error) {
			console.error("Failed to send StopPressed:", error);
		}
	}

	async function loadSettings() {
		try {
			const settings = await invoke<AlarmSettings>("get_alarm_settings");
			setSavedSettings(settings);
			setTimerHour(settings.alarm_hour);
			setTimerMinute(settings.alarm_minute);
			setAlarmIsOn(settings.alarm_is_on);
			console.log("Loaded settings:", settings);
		} catch (error) {
			console.error("Failed to load settings:", error);
		}
	}

	async function saveSettings() {
		try {
			await invoke("save_alarm_settings", {
				alarmHour: timerHour,
				alarmMinute: timerMinute,
				alarmIsOn: alarmIsOn,
			});
			console.log("Settings saved and sent to VRChat");
			// 設定を再読み込み
			await loadSettings();
		} catch (error) {
			console.error("Failed to save settings:", error);
		}
	}

	async function loadAndSendSettings() {
		try {
			const settings = await invoke<AlarmSettings>("load_and_send_settings");
			setSavedSettings(settings);
			console.log("Loaded and sent settings to VRChat:", settings);
		} catch (error) {
			console.error("Failed to load and send settings:", error);
		}
	}

	useEffect(() => {
		// 初回読み込み
		fetchAppState();
		loadSettings();
		
		// 1秒ごとに状態を更新
		const interval = setInterval(fetchAppState, 1000);
		
		return () => clearInterval(interval);
	}, []);

	return (
		<main className="container">
			<h1>VRC OSC Alarm - Parameter Monitor</h1>

			<div style={{ border: '1px solid #ccc', padding: '10px', margin: '10px 0' }}>
				<h2>Received VRC Parameters</h2>
				{appState ? (
					<div>
						<p><strong>Alarm Set Hour:</strong> {appState.alarm_set_hour}</p>
						<p><strong>Alarm Set Minute:</strong> {appState.alarm_set_minute}</p>
						<p><strong>Alarm Is On:</strong> {appState.alarm_is_on ? 'ON' : 'OFF'}</p>
						<p><strong>Snooze Pressed:</strong> {appState.snooze_pressed ? 'YES' : 'NO'}</p>
						<p><strong>Stop Pressed:</strong> {appState.stop_pressed ? 'YES' : 'NO'}</p>
						<p><strong>Last OSC Received:</strong> {appState.last_osc_received || 'None'}</p>
					</div>
				) : (
					<p>Loading...</p>
				)}
			</div>

			<div style={{ border: '1px solid #ccc', padding: '10px', margin: '10px 0' }}>
				<h2>Alarm Settings</h2>
				
				{savedSettings && (
					<div style={{ marginBottom: '15px', padding: '10px', backgroundColor: '#f0f0f0' }}>
						<h3>Current Saved Settings</h3>
						<p>Time: {savedSettings.alarm_hour}:{savedSettings.alarm_minute.toString().padStart(2, '0')} | Enabled: {savedSettings.alarm_is_on ? 'Yes' : 'No'}</p>
					</div>
				)}

				<div style={{ marginBottom: '15px' }}>
					<h3>Set Alarm Time</h3>
					<div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginBottom: '10px' }}>
						<label>Hour:</label>
						<input
							type="number"
							value={timerHour}
							onChange={(event) => setTimerHour(Number(event.target.value))}
							min="0"
							max="23"
							style={{ width: '60px' }}
						/>
						<label>Minute:</label>
						<input
							type="number"
							value={timerMinute}
							onChange={(event) => setTimerMinute(Number(event.target.value))}
							min="0"
							max="59"
							style={{ width: '60px' }}
						/>
						<label>
							<input
								type="checkbox"
								checked={alarmIsOn}
								onChange={(event) => setAlarmIsOn(event.target.checked)}
							/>
							Alarm Enabled
						</label>
					</div>
					<div style={{ display: 'flex', gap: '10px' }}>
						<button 
							onClick={saveSettings}
							style={{ padding: '10px 15px', backgroundColor: '#51cf66', color: 'white' }}
						>
							💾 Save Settings & Send to VRC
						</button>
						<button 
							onClick={loadSettings}
							style={{ padding: '10px 15px', backgroundColor: '#339af0', color: 'white' }}
						>
							📂 Load Settings
						</button>
						<button 
							onClick={loadAndSendSettings}
							style={{ padding: '10px 15px', backgroundColor: '#fd7e14', color: 'white' }}
						>
							📤 Load & Send to VRC
						</button>
					</div>
				</div>

				<div style={{ marginBottom: '15px' }}>
					<h3>Manual Send Controls</h3>
					<div style={{ display: 'flex', gap: '10px', marginBottom: '10px' }}>
						<button onClick={() => sendAlarmSetHour(timerHour)}>Send Hour</button>
						<button onClick={() => sendAlarmSetMinute(timerMinute)}>Send Minute</button>
						<button onClick={() => sendAlarmIsOn(alarmIsOn)}>Send Alarm Status</button>
					</div>
				</div>

				<div style={{ marginBottom: '15px' }}>
					<h3>Button Controls</h3>
					<div style={{ display: 'flex', gap: '10px' }}>
						<button 
							onClick={() => sendSnoozePressed(true)}
							style={{ padding: '5px 10px', backgroundColor: '#ffd43b', color: 'black' }}
						>
							Press Snooze
						</button>
						<button 
							onClick={() => sendStopPressed(true)}
							style={{ padding: '5px 10px', backgroundColor: '#ff8787', color: 'white' }}
						>
							Press Stop
						</button>
					</div>
				</div>
			</div>

			<div style={{ border: '1px solid #ccc', padding: '10px', margin: '10px 0' }}>
				<h2>VRC Alarm Control</h2>
				<div style={{ marginBottom: '10px' }}>
					<button 
						onClick={() => sendAlarmShouldFire(true)}
						style={{ margin: '5px', padding: '10px', backgroundColor: '#ff6b6b', color: 'white' }}
					>
						Send Alarm ON
					</button>
					<button 
						onClick={() => sendAlarmShouldFire(false)}
						style={{ margin: '5px', padding: '10px', backgroundColor: '#51cf66', color: 'white' }}
					>
						Send Alarm OFF
					</button>
				</div>
			</div>

			<div>
				<button onClick={sendOsc}>Send Custom OSC</button>
				<p>{sendOscMsg}</p>
			</div>
		</main>
	);
}

export default App;
