# タスクトレイ常駐とGUIウィンドウのライフサイクル・メモリ管理仕様書

本ドキュメントでは、常にバックグラウンドで動作し、必要に応じてGUIを表示する**「タスクトレイ常駐型アプリケーション」**において、メモリ消費を最小限に抑えつつ、スムーズなウィンドウ表示制御を実現するためのライフサイクル仕様と実装サンプル（Rust + Tauri v2）について解説します。

---

## 1. ライフサイクル設計の基本コンセプト

Web技術を用いたデスクトップアプリフレームワーク（Tauri, Electronなど）は、WebRender（ブラウザエンジン）を内包するため、通常のネイティブアプリに比べてメモリ消費量が多くなりがちです。
本仕様では、以下の3つのアプローチで「軽量な常駐」と「軽快なUI表示」を両立します。

### ① 非表示（Hide）ではなく「ウィンドウの破棄（Close）」
- 一般的なアプリではウィンドウを「閉じる」または「最小化する」際、単に `hide()` 処理を行い裏でインスタンスを保持しますが、これでは数十MB〜数百MBのメモリが消費されたままになります。
- 本仕様では、ウィンドウを閉じるときや最小化する際、ウィンドウ自体を**完全に破棄 (`close`)** し、メモリを即時解放します。

### ② 必要に応じた「ウィンドウの動的再生成」
- タスクトレイクリックやショートカットキーなどによって再表示が要求された際、メモリ上にウィンドウが存在しない（破棄されている）場合は、**その場でウィンドウ（WebView）を動的に再ビルド**して表示します。

### ③ 二重起動防止（シングルインスタンス）による制御移譲
- すでにアプリが常駐している状態でユーザーが二重にアプリを起動した場合、新たなプロセスを起動するのではなく、**常駐している既存のプロセスにシグナルとコマンドライン引数を渡し、既存プロセス側でウィンドウを動的に再生成・表示**します。

---

## 2. 詳細仕様

### 2.1. タスクトレイ常駐と終了処理
- **終了の抑止 (api.prevent_exit)**
  ウィンドウの「閉じる」ボタンが押されたり、OSのウィンドウ終了要求が発生しても、アプリケーションプロセス自体は終了させず、イベントをキャンセルします。
- **明示的終了**
  タスクトレイのコンテキストメニューから「終了」が選択された場合のみ、 `std::process::exit(0)` を呼び出し、プロセスを完全に終了します。

### 2.2. ウィンドウ破棄（メモリ解放）のトリガー

| ウィンドウ種類 | トリガーイベント | 処理内容 | 目的 |
| :--- | :--- | :--- | :--- |
| **メイン（設定）画面** | 最小化 (`is_minimized()`) | ウィンドウを破棄 (`close()`) | 常駐時に不要なWebViewメモリを解放する |
| **メイン（設定）画面** | 閉じるボタン押下 | ウィンドウを破棄 (`close()`) | 同上 |
| **ポップアップ（操作）画面** | フォーカス喪失 (`Focused(false)`) | ウィンドウを破棄 (`close()`) | 操作終了時に自動で消去しメモリを解放する |
| **全体（自動起動時）** | 起動時のコマンド引数に `--autostart` あり | メインウィンドウを生成せず、あるいは即座に破棄 | バックグラウンド起動時のメモリ無駄遣いを防ぐ |

### 2.3. 再表示の挙動
- タスクトレイのクリックイベント、またはホットキーイベントをトリガーとします。
- **ウィンドウの存在確認**:
  - `get_webview_window("label")` でインスタンスが存在するか確認します。
  - **存在する場合**: 表示 (`show`) ➡ 最小化解除 (`unminimize`) ➡ 最前面化 (`set_always_on_top(true)`) ➡ 最前面化解除 (`set_always_on_top(false)`) ➡ フォーカス強制 (`set_focus`) を順に行い、確実にユーザーの最前面へ引き出します。
  - **存在しない場合**: ウィンドウビルダー (`WebviewWindowBuilder`) を用いて、メモリ上にウィンドウを動的に構築し、作成完了後にフォーカスをあてて表示します。

---

## 3. 実装サンプルコード (Rust + Tauri v2)

以下は、この仕様に基づいたウィンドウライフサイクルおよびタスクトレイ管理の実装サンプルです。

### 1. タスクトレイの設定 (`src/tray.rs`)
```rust
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // トレイメニューの構築
    let settings_i = MenuItem::with_id(app, "settings", "設定画面を開く", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings_i, &quit_i])?;

    let icon = app.default_window_icon().cloned().unwrap();

    let _tray = TrayIconBuilder::new()
        .tooltip("常駐アプリケーション")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => {
                // 明示的な終了要求。プロセスをクリーンに終了します。
                std::process::exit(0);
            }
            "settings" => {
                show_main_window(app);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| match event {
            // トレイアイコンのクリック時にメインウィンドウを前面に表示
            TrayIconEvent::Click { button: MouseButton::Left, .. } => {
                show_main_window(tray.app_handle());
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

/// メインウィンドウを表示します（存在しない場合は動的に生成します）
pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        // すでにインスタンスがある場合は、最前面へ連れてくる
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_always_on_top(true);
        let _ = window.set_always_on_top(false);
        let _ = window.set_focus();
    } else {
        // メモリ解放されてインスタンスがない場合は、再構築する
        if let Ok(builder) = tauri::WebviewWindowBuilder::new(
            app,
            "main",
            tauri::WebviewUrl::App("index.html".into())
        )
        .title("設定画面")
        .inner_size(800.0, 600.0)
        .resizable(true)
        .visible(true)
        .build() {
            let _ = builder.set_always_on_top(true);
            let _ = builder.set_always_on_top(false);
            let _ = builder.set_focus();
        }
    }
}
```

### 2. アプリのエントリポイントとイベント制御 (`src/lib.rs` / `src/main.rs`)
```rust
use tauri::Manager;
pub mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let args: Vec<String> = std::env::args().collect();

            // トレイの初期化
            let _ = tray::setup_tray(app.handle());

            // 自動起動パラメータ（--autostart）がない場合のみ、初期起動時にUIを表示する
            if !args.contains(&"--autostart".to_string()) {
                tray::show_main_window(app.handle());
            } else {
                // 自動起動時は、不要な初期ウィンドウオブジェクトがあれば閉じてメモリを解放
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.close();
                }
            }
            Ok(())
        })
        // シングルインスタンス（二重起動防止）プラグイン
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // 後から別プロセスで起動された場合、引数を受け取って既存プロセスで画面を表示
            tray::show_main_window(app);
        }))
        // ウィンドウのイベントハンドリング
        .on_window_event(|window, event| {
            match event {
                // 1. フォーカスが外れた場合の処理 (例: ポップアップ画面)
                tauri::WindowEvent::Focused(focused) => {
                    if !focused && window.label() == "arrange_popup" {
                        // フォーカスアウトしたら即座に破棄してメモリを解放
                        let _ = window.close();
                    }
                }
                // 2. 最小化またはサイズ変更イベントの処理
                tauri::WindowEvent::Resized(_) => {
                    if window.label() == "main" {
                        // ユーザーがウィンドウを最小化したとき
                        if window.is_minimized().unwrap_or(false) {
                            // 最小化されたWebViewを破棄し、メモリ消費をほぼゼロにする
                            let _ = window.close();
                        }
                    }
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("Tauriアプリのビルド中にエラーが発生しました")
        .run(|_app_handle, event| match event {
            // OSやウィンドウ側からアプリ終了（Exit）が要求された場合、終了を阻止して常駐化
            tauri::RunEvent::ExitRequested { api, .. } => {
                api.prevent_exit();
            }
            _ => {}
        });
}
```

---

## 4. この設計のメリットと注意点

### メリット
1. **メモリ消費の大幅な削減**:
   常駐時のメモリ使用量を、WebView動作時の数分の一（数MB程度）まで削減できます。
2. **フォーカス制御の確実性**:
   一度破棄したウィンドウを再生成するため、ブラウザエンジンのキャッシュやDOMのゴミによるパフォーマンス低下を防ぎ、常にクリーンな状態のUIを表示できます。

### 注意点（実装時の考慮事項）
1. **フロントエンドの状態管理（State）の揮発**:
   ウィンドウを `close` すると、フロントエンドのメモリ上（JavaScriptの変数やReact/Vueの状態）はすべてクリアされます。
   - **対策**: 設定値などは破棄する前に必ずバックエンド（Rust側のメモリ、またはローカル設定ファイル）に保存し、ウィンドウ再生成時にフロントエンド側で再ロードする設計にしてください。
2. **ウィンドウ起動時のわずかな遅延**:
   毎回Webviewのプロセスから立ち上げるため、非表示から `show()` するだけに比べて数ミリ秒〜数百ミリ秒の起動遅延（白画面の一瞬のチラつきなど）が発生する可能性があります。
   - **対策**: Tauriの「ウィンドウ初期非表示設定」と、フロントエンドのDOM読み込み完了後の「プログラムによる可視化（`show()`）」を組み合わせることで、チラつきのないスムーズな表示が可能です。
