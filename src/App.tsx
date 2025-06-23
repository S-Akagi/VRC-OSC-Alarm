import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import "./App.css";

function App() {
  const [version, setVersion] = useState("");
  const [connectionStatus, setConnectionStatus] = useState("Disconnected");

  // 起動時の処理
  useEffect(() => {
    // バージョン取得
    invoke<string>("get_current_version").then(setVersion);

    // 接続状態を定期的にチェック
    const interval = setInterval(async () => {
      try {
        const state = await invoke<{ last_osc_received: string | null }>("get_current_state");
        if (state.last_osc_received) {
          const lastReceived = new Date(state.last_osc_received);
          const timeDiff = Date.now() - lastReceived.getTime();
          setConnectionStatus(timeDiff < 60000 ? "Connected" : "Disconnected");
        } else {
          setConnectionStatus("Disconnected");
        }
      } catch {
        setConnectionStatus("Disconnected");
      }
    }, 2000);

    return () => clearInterval(interval);
  }, []);

  const handleLinkOpen = () => {
    // BOOTHのURLは後で差し替えてください
    invoke("open", { path: "https://s-akagi0610.booth.pm/" });
  };

  const handleDrag = () => getCurrentWindow().startDragging();
  const handleClose = () => getCurrentWindow().close();

  return (
    <div className="app">
      <div className="custom-titlebar" onMouseDown={handleDrag}>
        <span className="window-title">AAS Gadget</span>
        <div className="titlebar-buttons">
          <button type="button" className="close-btn" onClick={handleClose}>×</button>
        </div>
      </div>

      <div className="main-content">
        <h1 className="app-name">AAS Gadget <span className="version">v{version}</span></h1>
        <div className="status-display">
          <span className={`status-indicator ${connectionStatus.toLowerCase()}`}>●</span>
          <span>{connectionStatus}</span>
        </div>
        <p className="description">
          時刻とPC情報をVRChatに送信中...
        </p>
        <button 
          type="button" 
          className="pro-link-btn"
          onClick={handleLinkOpen}
        >
          Pro版（アラーム機能など）はこちら
        </button>
      </div>
    </div>
  );
}

export default App;