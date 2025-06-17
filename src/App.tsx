// src/App.tsx

import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import "./App.css";
import { LogicalSize, Window, getCurrentWindow } from "@tauri-apps/api/window";

// === Type Definitions ===
interface AppState {
	last_osc_received: string | null;
	last_osc_sent: string | null;
	alarm_set_hour: number;
	alarm_set_minute: number;
	alarm_is_on: boolean;
	snooze_pressed: boolean;
	stop_pressed: boolean;
	is_ringing: boolean;
	snooze_count: number;
	max_snoozes: number;
	ringing_duration_minutes: number;
	snooze_duration_minutes: number;
}

interface AlarmSettings {
	alarm_hour: number;
	alarm_minute: number;
	alarm_is_on: boolean;
}

// === Main App Component ===
function App() {
	// === State Management ===
	const [timerHour, setTimerHour] = useState(7);
	const [timerMinute, setTimerMinute] = useState(0);
	const [appState, setAppState] = useState<AppState | null>(null);
	const [alarmIsOn, setAlarmIsOn] = useState(false);
	const [isExpanded, setIsExpanded] = useState(false);
	const [maxSnoozes, setMaxSnoozes] = useState(5);
	const [ringingDuration, setRingingDuration] = useState(15);
	const [snoozeDuration, setSnoozeDuration] = useState(9);

	// === Backend API Functions ===
	async function fetchAppState() {
		try {
			const state = await invoke<AppState>("get_current_state");
			setAppState(state);
		} catch (error) {
			console.error("Failed to fetch app state:", error);
		}
	}

	async function loadSettings() {
		try {
			const settings = await invoke<AlarmSettings>("get_alarm_settings");
			setTimerHour(settings.alarm_hour);
			setTimerMinute(settings.alarm_minute);
			setAlarmIsOn(settings.alarm_is_on);
		} catch (error) {
			console.error("Failed to load settings:", error);
		}
	}

	async function loadTimerSettings() {
		try {
			const [max, ringing, snooze] =
				await invoke<[number, number, number]>("get_timer_settings");
			setMaxSnoozes(max);
			setRingingDuration(ringing);
			setSnoozeDuration(snooze);
		} catch (error) {
			console.error("Failed to load timer settings:", error);
		}
	}

	async function saveAlarmSettings() {
		try {
			await invoke("save_alarm_settings", {
				alarmHour: timerHour,
				alarmMinute: timerMinute,
				alarmIsOn: alarmIsOn,
			});
			await loadSettings();
		} catch (error) {
			console.error("Failed to save settings:", error);
		}
	}

	async function saveTimerSettings() {
		try {
			await invoke("save_timer_settings", {
				maxSnoozes: maxSnoozes,
				ringingDurationMinutes: ringingDuration,
				snoozeDurationMinutes: snoozeDuration,
			});
			await loadTimerSettings();
		} catch (error) {
			console.error("Failed to save timer settings:", error);
		}
	}

	// === Window Management Functions ===
	const handleWindowDrag = async (e: React.MouseEvent) => {
		// Only start dragging if clicking on the titlebar content, not buttons
		if ((e.target as HTMLElement).closest(".titlebar-buttons")) {
			return;
		}

		try {
			const appWindow = getCurrentWindow();
			await appWindow.startDragging();
		} catch (error) {
			console.error("Failed to start dragging:", error);
		}
	};

	const handleMinimize = async (e: React.MouseEvent) => {
		e.stopPropagation();
		e.preventDefault();

		try {
			const appWindow = getCurrentWindow();
			await appWindow.minimize();
		} catch (error) {
			console.error("Failed to minimize window:", error);
		}
	};

	const handleClose = async (e: React.MouseEvent) => {
		e.stopPropagation();
		e.preventDefault();

		try {
			const appWindow = getCurrentWindow();
			await appWindow.close();
		} catch (error) {
			console.error("Failed to close window:", error);
		}
	};


	const updateWindowSize = useCallback(async () => {
		const appWindow = await Window.getByLabel("main");
		if (appWindow) {
			let height = 80; // Base height (28px titlebar + 53px content)

			if (appState?.is_ringing) {
				height += 28; // Add space for ringing alert
			}

			if (isExpanded) {
				height += 138; // Add space for settings panel

				// Check if advanced settings are open
				const advancedDetails = document.querySelector(".settings-details");
				if (advancedDetails && advancedDetails.hasAttribute("open")) {
					height += 120; // Add space for advanced settings
				}
			}

			console.log("Updating window size to:", height);
			await appWindow.setSize(new LogicalSize(220, height));
		}
	}, [isExpanded, appState?.is_ringing]);

	// === Utility Functions ===
	const formatTime = (hour: number, minute: number) => {
		return `${hour.toString().padStart(2, "0")}:${minute.toString().padStart(2, "0")}`;
	};

	const getStatusColor = () => {
		if (appState?.is_ringing) return "#ff4757";
		if (alarmIsOn) return "#2ed573";
		return "#747d8c";
	};

	const getConnectionStatus = () => {
		if (!appState?.last_osc_received) return "Disconnected";
		const lastReceived = new Date(appState.last_osc_received);
		const timeDiff = Date.now() - lastReceived.getTime();
		return timeDiff < 10000 ? "Connected" : "Disconnected";
	};

	// === Effects ===
	useEffect(() => {
		updateWindowSize();
	}, [updateWindowSize]);

	useEffect(() => {
		loadSettings();
		loadTimerSettings();
		fetchAppState();

		const interval = setInterval(fetchAppState, 1000);
		return () => clearInterval(interval);
	}, []);

	// === Render ===
	return (
		<div className="app">
			{/* Custom Titlebar */}
			<div
				className="custom-titlebar"
				onMouseDown={handleWindowDrag}
			>
				<div className="titlebar-content">
					<span className="window-title">VRC Alarm</span>
					<div className="titlebar-buttons">
						<button
							className="titlebar-btn minimize-btn"
							onClick={handleMinimize}
							title="Minimize"
							aria-label="Minimize window"
						>
							−
						</button>
						<button
							className="titlebar-btn close-btn"
							onClick={handleClose}
							title="Close"
							aria-label="Close window"
						>
							×
						</button>
					</div>
				</div>
			</div>

			{/* Main Alarm Display */}
			<div className="alarm-display">
				<div className="alarm-time">{formatTime(timerHour, timerMinute)}</div>
				<div className="alarm-status">
					<div
						className="status-dot"
						style={{ backgroundColor: getStatusColor() }}
					></div>
					<span className="status-text">
						{appState?.is_ringing ? "RINGING" : alarmIsOn ? "ON" : "OFF"}
					</span>
				</div>
				<button
					className="expand-btn"
					onClick={() => setIsExpanded(!isExpanded)}
				>
					{isExpanded ? "−" : "+"}
				</button>
			</div>

			{/* Ringing Alert */}
			{appState?.is_ringing && (
				<div className="ringing-alert">
					Snooze {appState.snooze_count}/{appState.max_snoozes}
				</div>
			)}

			{/* Settings Panel */}
			{isExpanded && (
				<div className="settings-panel">
					{/* Quick Controls */}
					<div className="quick-controls">
						<div className="time-setting">
							<input
								type="number"
								value={timerHour}
								onChange={(e) => setTimerHour(Number(e.target.value))}
								min="0"
								max="23"
								className="time-input-small"
							/>
							<span>:</span>
							<input
								type="number"
								value={timerMinute}
								onChange={(e) => setTimerMinute(Number(e.target.value))}
								min="0"
								max="59"
								className="time-input-small"
							/>
						</div>
						<label className="toggle-small">
							<input
								type="checkbox"
								checked={alarmIsOn}
								onChange={(e) => setAlarmIsOn(e.target.checked)}
							/>
							<span className="toggle-slider-small"></span>
						</label>
						<button onClick={saveAlarmSettings} className="save-btn">
							Save
						</button>
					</div>

					{/* Advanced Settings */}
					<details className="settings-details" onToggle={updateWindowSize}>
						<summary>Advanced</summary>
						<div className="advanced-settings">
							<div className="setting-item">
								<label>Max Snoozes:</label>
								<input
									type="number"
									value={maxSnoozes}
									onChange={(e) => setMaxSnoozes(Number(e.target.value))}
									min="1"
									max="20"
									className="setting-input-small"
								/>
							</div>
							<div className="setting-item">
								<label>Ring Duration (min):</label>
								<input
									type="number"
									value={ringingDuration}
									onChange={(e) => setRingingDuration(Number(e.target.value))}
									min="1"
									max="60"
									className="setting-input-small"
								/>
							</div>
							<div className="setting-item">
								<label>Snooze Duration (min):</label>
								<input
									type="number"
									value={snoozeDuration}
									onChange={(e) => setSnoozeDuration(Number(e.target.value))}
									min="1"
									max="30"
									className="setting-input-small"
								/>
							</div>
							<button onClick={saveTimerSettings} className="save-btn-small">
								Save Timer Settings
							</button>
						</div>
					</details>

					{/* Connection Status */}
					<div className="connection-status-compact">
						<span
							className={`connection-indicator ${getConnectionStatus().toLowerCase()}`}
						>
							● {getConnectionStatus()}
						</span>
					</div>
				</div>
			)}
		</div>
	);
}

export default App;
