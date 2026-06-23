/// アプリケーション全体で使用する定数定義
/// 変更頻度の低い識別子・タイミング値を集約し、マジックナンバーを排除する

/// アプリケーション名（ウィンドウタイトル、トレイツールチップ等で使用）
pub const APP_NAME: &str = "MicSplitter";

/// 設定ファイルのパス
pub const CONFIG_FILE: &str = "config.json";

/// 単一インスタンス制限用の Named Mutex 名
pub const MUTEX_NAME: &str = "Global\\MicSplitterMutex";

/// 二重起動検知用の IPC アドレス（UDP）
pub const IPC_ADDR: &str = "127.0.0.1:45123";

/// デバイス接続/切断の自動検知ポーリング間隔（秒）
pub const DEVICE_POLL_INTERVAL_SECS: u64 = 2;

/// イベントループのスリープ間隔（ミリ秒）
pub const EVENT_LOOP_INTERVAL_MS: u64 = 50;

/// オーディオストリームのレイテンシ（ミリ秒）
pub const AUDIO_LATENCY_MS: f32 = 50.0;
