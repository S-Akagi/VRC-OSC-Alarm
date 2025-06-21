# VRChatどこでもアラーム

VRChat内でアラーム機能を提供するOSCアプリケーション

## 機能

- VRChat内でアラーム時刻の設定・変更
- アラームのオン/オフ切り替え
- スヌーズ機能（最大20回まで設定可能）
- VRChatとリアルタイム同期
- 自動アップデート通知

## 動作環境

- Windows 10/11
- VRChat（OSC機能有効）

## セットアップ

1. [Releases](https://s-akagi0610.booth.pm/)から最新版をダウンロード
2. VRChatでOSC機能を有効化
3. アプリケーションを起動

## 使用方法

1. アプリケーション起動後、VRChatと自動的に接続
2. VRChat内のアバターパラメータでアラーム設定
3. 設定は自動的にアプリと同期

## 開発者向け連携仕様

本ツールはVRChat向けアラーム機能のコア機能を提供します。アバター制作者やワールド制作者は以下のOSCパラメータを活用して時計やアラーム表示を作成できます。

### OSCパラメータ仕様

| パラメータ名 | 型 | 方向 | 説明 |
|------------|----|----|-----|
| AlarmSetHour | Float | 双方向 | アラーム時間（0-1の範囲、0=0時、1=23時） |
| AlarmSetMinute | Float | 双方向 | アラーム分（0-1の範囲、0=0分、1=59分） |
| AlarmIsOn | Bool | 双方向 | アラーム有効/無効状態 |
| SnoozePressed | Bool | VRC→App | スヌーズボタンの押下状態 |
| StopPressed | Bool | VRC→App | 停止ボタンの押下状態 |
| AlarmShouldFire | Bool | App→VRC | アラーム発火中の状態 |

### 活用例

#### 時計表示の実装
```
AlarmSetHour × 23 = 表示時間（整数部分）
AlarmSetMinute × 59 = 表示分（整数部分）
AlarmIsOn = ON/OFF表示の切り替え
```

## システム構成図

```mermaid
graph TB
    subgraph "VRChatどこでもアラーム App"
        UI[UI Layer<br/>React Frontend]
        Core[Core Logic<br/>Rust Backend]
        Timer[Timer Manager]
        Config[Settings Storage]
    end
    
    subgraph "VRChat"
        Avatar[Avatar Parameters]
        World[World Objects]
        OSC_VRC[OSC Receiver<br/>Port 9001]
    end
    
    subgraph "Network"
        OSC_OUT[OSC Sender<br/>Port 9000]
        HB[Heartbeat<br/>30s interval]
    end
    
    UI <--> Core
    Core <--> Timer
    Core <--> Config
    Core <--> OSC_OUT
    OSC_OUT <--> OSC_VRC
    Avatar <--> OSC_VRC
    World <--> OSC_VRC
    
    HB -.-> OSC_OUT
    
    style UI fill:#e1f5fe
    style Core fill:#f3e5f5
    style Avatar fill:#e8f5e8
    style World fill:#e8f5e8
```

## データフロー図

```mermaid
sequenceDiagram
    participant UI as React UI
    participant Core as Rust Core
    participant VRC as VRChat
    participant Timer as Timer Manager
    
    Note over UI,Timer: アラーム設定時
    UI->>Core: save_alarm_settings(hour, minute, is_on)
    Core->>Core: Validate & clamp values
    Core->>VRC: Send OSC parameters
    Core->>Timer: Calculate next alarm
    Core->>UI: Emit settings changed event
    
    Note over UI,Timer: VRCからの設定変更
    VRC->>Core: OSC AlarmSetHour/Minute/IsOn
    Core->>Core: Validate & save settings
    Core->>VRC: Send back if clamped
    Core->>UI: Emit settings changed event
    Core->>Timer: Recalculate alarm
    
    Note over UI,Timer: アラーム発火
    Timer->>Core: Alarm triggered
    Core->>VRC: AlarmShouldFire = true
    Core->>UI: Update ringing state
    
    Note over UI,Timer: スヌーズ/停止
    VRC->>Core: SnoozePressed/StopPressed = true
    Core->>Timer: Handle snooze/stop event
    Core->>VRC: AlarmShouldFire = false
    Core->>UI: Update state
    
    Note over UI,Timer: ハートビート
    loop Every 30 seconds
        Core->>VRC: Send current settings bundle
    end
```

## 状態管理図

```mermaid
stateDiagram-v2
    [*] --> Disconnected
    Disconnected --> Connected: OSC受信
    Connected --> Disconnected: 60秒間無通信
    
    state Connected {
        [*] --> AlarmOff
        AlarmOff --> AlarmOn: AlarmIsOn = true
        AlarmOn --> AlarmOff: AlarmIsOn = false
        
        state AlarmOn {
            [*] --> Waiting
            Waiting --> Ringing: 設定時刻到達
            Ringing --> Snoozed: SnoozePressed
            Ringing --> Stopped: StopPressed
            Snoozed --> Ringing: スヌーズ時間経過
            Snoozed --> Stopped: 最大スヌーズ回数到達
            Stopped --> Waiting: 次のアラーム計算
        }
    }
```

## ライセンス

MIT License

## 免責事項

本ソフトウェアはVRChatの非公式ツールです。VRChat Inc.とは関係ありません。