import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import "./App.css";
import { LogicalSize, Window, getCurrentWindow } from "@tauri-apps/api/window";

// アプリの状態
interface AppState {
  last_osc_received: string | null; // OSC受信時間
  last_osc_sent: string | null; // OSC送信時間
  alarm_set_hour: number; // アラーム時間
  alarm_set_minute: number; // アラーム分
  alarm_is_on: boolean; // アラームがオンかどうか
  snooze_pressed: boolean; // スヌーズボタンが押されたかどうか
  stop_pressed: boolean; // ストップボタンが押されたかどうか
  is_ringing: boolean; // アラームが鳴っているかどうか
  snooze_count: number; // スヌーズ回数
  max_snoozes: number; // 最大スヌーズ回数
  ringing_duration_minutes: number; // アラーム時間
  snooze_duration_minutes: number; // スヌーズ間隔
}

// アラーム設定の型
interface AlarmSettings {
  alarm_hour: number; // アラーム時間
  alarm_minute: number; // アラーム分
  alarm_is_on: boolean; // アラームがオンかどうか
}

// アップデート情報の型
interface UpdateInfo {
  current_version: string;
  latest_version: string;
  has_update: boolean;
  download_url: string;
}

// メインアプリコンポーネント
function App() {
  const [timerHour, setTimerHour] = useState(7); // アラーム時間
  const [timerMinute, setTimerMinute] = useState(0); // アラーム分
  const [appState, setAppState] = useState<AppState | null>(null); // アプリの状態
  const [alarmIsOn, setAlarmIsOn] = useState(false); // アラームがオンかどうか
  const [isExpanded, setIsExpanded] = useState(false); // 設定パネルが展開されているかどうか
  const [maxSnoozes, setMaxSnoozes] = useState(5); // 最大スヌーズ回数
  const [ringingDuration, setRingingDuration] = useState(15); // アラーム時間
  const [snoozeDuration, setSnoozeDuration] = useState(9); // スヌーズ間隔
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null); // アップデート情報

  // アプリの状態を取得
  const fetchAppState = async () => {
    try {
      const state = await invoke<AppState>("get_current_state");
      setAppState(state);
    } catch {
      // バックエンドとの通信エラーは一時的なものが多いため、状態をnullに設定して続行
      setAppState(null);
    }
  };

  // アラーム設定を読み込む
  async function loadSettings() {
    const settings = await invoke<AlarmSettings>("get_alarm_settings");
    setTimerHour(settings.alarm_hour);
    setTimerMinute(settings.alarm_minute);
    setAlarmIsOn(settings.alarm_is_on);
  }

  // タイマー設定を読み込む
  async function loadTimerSettings() {
    const [max, ringing, snooze] = await invoke<[number, number, number]>("get_timer_settings");
    setMaxSnoozes(max);
    setRingingDuration(ringing);
    setSnoozeDuration(snooze);
  }

  // アラーム設定を保存
  async function saveAlarmSettings() {
    await invoke("save_alarm_settings", {
      alarmHour: timerHour,
      alarmMinute: timerMinute,
      alarmIsOn,
    });
  }

  // タイマー設定を保存
  async function saveTimerSettings() {
    await invoke("save_timer_settings", {
      maxSnoozes,
      ringingDurationMinutes: ringingDuration,
      snoozeDurationMinutes: snoozeDuration,
    });
  }

  // アップデート確認
  async function checkForUpdates() {
    try {
      const update = await invoke<UpdateInfo>("check_for_updates");
      setUpdateInfo(update);
      if (update.has_update) {
        await showUpdateDialog(update);
      }
    } catch (error) {
      console.error("アップデート確認に失敗しました:", error);
    }
  }

  // ネイティブダイアログでアップデート通知
  async function showUpdateDialog(update: UpdateInfo) {
    try {
      const { confirm } = await import("@tauri-apps/plugin-dialog");
      const result = await confirm(
        `新しいバージョンが利用可能です。\n\n現在のバージョン: v${update.current_version}\n最新バージョン: v${update.latest_version}\n\nダウンロードページを開きますか？`,
        { title: "アップデート通知", kind: "info" },
      );

      if (result) {
        window.open(update.download_url, "_blank");
      }
    } catch (error) {
      console.error("ダイアログ表示に失敗しました:", error);
    }
  }

  // ライセンス情報表示
  async function showLicenseInfo() {
    try {
      const { message } = await import("@tauri-apps/plugin-dialog");
      const licenseText = `VRC OSC Alarm v0.1.0

このソフトウェアは以下のオープンソースライブラリを使用しています：

【主要フレームワーク】
• Tauri Framework (MIT/Apache-2.0)
• React (MIT)
• TypeScript (Apache-2.0)

【Rustライブラリ】
• Tokio (MIT) - 非同期ランタイム
• Serde (MIT/Apache-2.0) - シリアライゼーション
• Chrono (MIT/Apache-2.0) - 日時処理
• ROSC (MIT/Apache-2.0) - OSC通信
• Reqwest (MIT/Apache-2.0) - HTTP通信

使用している全ライブラリは MIT, Apache-2.0, MPL-2.0 ライセンスの下で提供されており、すべて商用利用が許可されています。

本ソフトウェアはVRChatの非公式ツールです。
VRChat Inc. とは関係ありません。`;

      await message(licenseText, { 
        title: "ライセンス情報", 
        kind: "info" 
      });
    } catch (error) {
      console.error("ライセンス情報表示に失敗しました:", error);
    }
  }

  // ウィンドウをドラッグ
  const handleWindowDrag = async (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest(".titlebar-buttons")) return;

    await getCurrentWindow().startDragging();
  };


  // ウィンドウを最小化
  const handleMinimize = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await getCurrentWindow().minimize();
  };

  // ウィンドウを閉じる
  const handleClose = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await getCurrentWindow().close();
  };

  // ウィンドウサイズを更新
  const updateWindowSize = useCallback(async () => {
    const appWindow = await Window.getByLabel("main");
    if (!appWindow) return;

    let height = 80;
    if (appState?.is_ringing) height += 28; // アラーム中の場合は28px追加
    if (isExpanded) {
      height += 148; // 基本設定パネル + ライセンステキスト分（138 + 10）
      const advancedDetails = document.querySelector(".settings-details");
      if (advancedDetails?.hasAttribute("open")) height += 120; // 詳細設定が開いている場合は120px追加
    }
    await appWindow.setSize(new LogicalSize(220, height)); // ウィンドウサイズを更新
  }, [isExpanded, appState?.is_ringing]);

  // 時間をフォーマット
  const formatTime = (hour: number, minute: number) => {
    return `${hour.toString().padStart(2, "0")}:${minute.toString().padStart(2, "0")}`; // 時間をフォーマット
  };

  // ステータスの色を取得
  const getStatusColor = () => {
    if (appState?.is_ringing) return "#ff4757"; // アラーム中の場合は赤
    if (alarmIsOn) return "#2ed573"; // アラームがオンの場合は緑
    return "#747d8c"; // それ以外はグレー
  };

  // 接続状態を取得
  const getConnectionStatus = () => {
    if (!appState?.last_osc_received) return "Disconnected"; // 接続がない場合は未接続
    const lastReceived = new Date(appState.last_osc_received);
    const timeDiff = Date.now() - lastReceived.getTime();
    return timeDiff < 60000 ? "Connected" : "Disconnected"; // 60秒以内に受信した場合は接続中
  };

  // ウィンドウサイズを更新
  useEffect(() => {
    updateWindowSize(); // ウィンドウサイズを更新
  }, [updateWindowSize]);

  // 初期化処理
  // biome-ignore lint/correctness/useExhaustiveDependencies: 初期化処理は一度だけ実行するため空の依存配列を使用
  useEffect(() => {
    loadSettings(); // アラーム設定を読み込む
    loadTimerSettings(); // タイマー設定を読み込む
    fetchAppState(); // アプリの状態を取得

    // 5秒後にアップデート確認（起動完了後に実行）
    setTimeout(checkForUpdates, 5000);

    const interval = setInterval(fetchAppState, 1000); // 1秒ごとにアプリの状態を取得

    // VRCからの設定変更イベントをリッスン
    const unlistenAlarmSettings = listen<AlarmSettings>("alarm-settings-changed", (event) => {
      const settings = event.payload;
      setTimerHour(settings.alarm_hour);
      setTimerMinute(settings.alarm_minute);
      setAlarmIsOn(settings.alarm_is_on);
    });

    return () => {
      clearInterval(interval); // コンポーネントがアンマウントされたらインターバルをクリア
      unlistenAlarmSettings.then((unlisten) => unlisten()); // イベントリスナーも解除
    };
  }, []);

  return (
    <div className="app">
      {/* カスタムタイトルバー */}
      <div className="custom-titlebar" onMouseDown={handleWindowDrag}>
        <div className="titlebar-content">
          <span className="window-title">VRC OSC Alarm System</span>
          <div className="titlebar-buttons">
            <button
              type="button"
              className="titlebar-btn minimize-btn"
              onClick={handleMinimize}
              title="最小化"
              aria-label="最小化"
            >
              −
            </button>
            <button
              type="button"
              className="titlebar-btn close-btn"
              onClick={handleClose}
              title="閉じる"
              aria-label="閉じる"
            >
              ×
            </button>
          </div>
        </div>
      </div>

      {/* メインアラーム表示 */}
      <div className="alarm-display">
        <div className="alarm-time">{formatTime(timerHour, timerMinute)}</div>
        <div className="alarm-status">
          <div className="status-dot" style={{ backgroundColor: getStatusColor() }} />
          <span className="status-text">{appState?.is_ringing ? "アラーム中" : alarmIsOn ? "オン" : "オフ"}</span>
        </div>
        <button type="button" className="expand-btn" onClick={() => setIsExpanded(!isExpanded)}>
          {isExpanded ? "−" : "+"}
        </button>
      </div>

      {/* アラーム通知 */}
      {(appState?.is_ringing || (appState?.snooze_count ?? 0) > 0) && (
        <div className="ringing-alert">
          スヌーズ {appState?.snooze_count ?? 0}/{appState?.max_snoozes ?? 0}
        </div>
      )}

      {/* 設定パネル */}
      {isExpanded && (
        <div className="settings-panel">
          {/* クイックコントロール */}
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
              <input type="checkbox" checked={alarmIsOn} onChange={(e) => setAlarmIsOn(e.target.checked)} />
              <span className="toggle-slider-small" />
            </label>
            <button type="button" onClick={saveAlarmSettings} className="save-btn">
              保存
            </button>
          </div>

          {/* 詳細設定 */}
          <details className="settings-details" onToggle={updateWindowSize}>
            <summary>詳細設定</summary>
            <div className="advanced-settings">
              <div className="setting-item">
                <label htmlFor="max-snoozes">最大スヌーズ回数:</label>
                <input
                  id="max-snoozes"
                  type="number"
                  value={maxSnoozes}
                  onChange={(e) => setMaxSnoozes(Number(e.target.value))}
                  min="1"
                  max="20"
                  className="setting-input-small"
                />
              </div>
              <div className="setting-item">
                <label htmlFor="ring-duration">アラーム時間 (分):</label>
                <input
                  id="ring-duration"
                  type="number"
                  value={ringingDuration}
                  onChange={(e) => setRingingDuration(Number(e.target.value))}
                  min="1"
                  max="60"
                  className="setting-input-small"
                />
              </div>
              <div className="setting-item">
                <label htmlFor="snooze-duration">スヌーズ間隔 (分):</label>
                <input
                  id="snooze-duration"
                  type="number"
                  value={snoozeDuration}
                  onChange={(e) => setSnoozeDuration(Number(e.target.value))}
                  min="1"
                  max="30"
                  className="setting-input-small"
                />
              </div>
              <button type="button" onClick={saveTimerSettings} className="save-btn-small">
                タイマー設定保存
              </button>
            </div>
          </details>

          {/* 接続状態とライセンス */}
          <div className="connection-status-compact">
            <span className={`connection-indicator ${getConnectionStatus().toLowerCase()}`}>
              ● {getConnectionStatus() === "Connected" ? "接続中" : "未接続"}
            </span>
            <button type="button" onClick={showLicenseInfo} className="license-btn">
              ライセンス情報
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
