# Windowsにおける自動起動登録と管理者権限制御の技術仕様書

本ドキュメントでは、Windowsアプリケーションにおいて**管理者権限（UAC昇格状態）を維持したまま自動起動（スタートアップ）を登録・管理する方法**、および**管理者権限の判定・自己昇格ロジック**について解説します。
他のアプリケーションに容易に転用・移植できるように、仕様とサンプルコード（Rust）を整理しています。

---

## 1. 全体アーキテクチャとアプローチ

### 1.1. 管理者権限での自動起動の課題
Windowsの通常のスタートアップ方法には以下の2つがあります。
- スタートアップフォルダへのショートカット配置 (`shell:startup`)
- レジストリの `Run` キーへの登録 (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`)

しかし、これらを経由して起動されたアプリケーションは、UAC（ユーザーアカウント制御）によって**標準ユーザー権限**に制限されて起動します。管理者権限が必要な処理を含むアプリケーションの場合、自動起動時に権限不足でエラーになるか、毎回UACプロンプトが表示されて自動起動が阻害されます。

### 1.2. 解決策：タスクスケジューラの活用
この課題を解決するため、Windowsの**「タスクスケジューラ」**を利用します。
タスクの構成で「最上位の特権で実行する」を設定し、トリガーを「ログオン時」とすることで、ユーザーがログインしたタイミングで自動的に管理者権限（UACプロンプトなし）でアプリケーションを起動させることができます。

---

## 2. コマンド・API仕様

### 2.1. タスクスケジューラへの登録 (schtasks)
自動起動の有効・無効化は、Windows標準コマンドの `schtasks.exe` を背後で実行することで行います。

#### タスクの登録コマンド
```cmd
schtasks /Create /F /TN "{TaskName}" /TR "\"{ExePath}\" --autostart" /SC ONLOGON /RL HIGHEST
```
- `/Create`: 新規タスクを作成します。
- `/F`: 同名のタスクが既に存在する場合、強制的に上書きします。
- `/TN {TaskName}`: タスクを識別する一意の名前を指定します。
- `/TR "\"{ExePath}\" --autostart"`: 実行するプログラムのフルパスと引数を指定します。パスにスペースが含まれる場合に対応するため、ダブルクォーテーションで囲みます。
  - `--autostart` 引数を付与することで、アプリ側で「自動起動時された時はGUIを表示せずシステムトレイに常駐する」といった制御が可能になります。
- `/SC ONLOGON`: ユーザーのログオン時に実行するトリガーを設定します。
- `/RL HIGHEST`: **「最上位の特権（管理者権限）」**で実行するように設定します。

#### タスクの削除（自動起動の解除）コマンド
```cmd
schtasks /Delete /F /TN "{TaskName}"
```
- `/Delete`: タスクを削除します。
- `/F`: 確認プロンプトを表示せずに強制削除します。

---

### 2.2. 管理者権限の判定と自己昇格 (UAC要求)
アプリケーション起動時に管理者権限があるかをチェックし、ない場合はユーザーにUACプロンプトを提示して管理者権限で自身を再起動します。

1. **管理者権限の判定**:
   Windows APIの `OpenProcessToken` を使用してプロセストークンを取得し、 `GetTokenInformation` で `TokenElevation` 情報を取得します。これにより、現在プロセスが昇格された特権で動作しているかを判定します。
2. **自己昇格再起動**:
   Windows APIの `ShellExecuteW` を呼び出します。その際、動詞 (verb) として `"runas"` を指定することで、WindowsがUACダイアログを表示し、ユーザーが承認すれば管理者権限で新しいプロセスが起動します。起動後、元のプロセスは直ちに終了します。

---

### 2.3. 環境変数 `Path` への登録（おまけ機能）
アプリケーションをコマンドラインやPowerShellから直接実行できるようにするため、ユーザー環境変数 `Path` にアプリの配置ディレクトリを追加・削除する機能です。
PowerShellの環境変数操作 API を用いて、安全に既存の `Path` に追加・削除します。

- **登録処理**:
  ```powershell
  $currentPath = [Environment]::GetEnvironmentVariable('Path', 'User');
  if ($currentPath -split ';' -notcontains '{BinDir}') {
      $newPath = $currentPath + ';' + '{BinDir}';
      [Environment]::SetEnvironmentVariable('Path', $newPath, 'User');
  }
  ```
- **解除処理**:
  ```powershell
  $currentPath = [Environment]::GetEnvironmentVariable('Path', 'User');
  $parts = $currentPath -split ';';
  $newParts = $parts | Where-Object { $_ -ne '{BinDir}' };
  $newPath = $newParts -join ';';
  [Environment]::SetEnvironmentVariable('Path', $newPath, 'User');
  ```

---

## 3. 転用可能なサンプルコード (Rust)

以下は、`windows` クレート（旧 `windows-rs`）および標準ライブラリを使用した、自動起動管理と管理者権限制御のモジュール実装サンプルです。

### `Cargo.toml` の依存関係設定
```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }

[target.'cfg(windows)'.dependencies]
windows = { version = "0.52", features = [
    "Win32_Foundation",
    "Win32_Security",
    "Win32_System_Threading",
    "Win32_UI_Shell",
    "Win32_UI_WindowsAndMessaging",
] }
```

### 実装コード: `src/admin.rs`
```rust
use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNOACTIVATE;

// バックグラウンドでコマンドを実行する際、黒いコンソールウィンドウを出さないためのフラグ
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// 現在のプロセスが管理者権限（昇格状態）で実行されているか判定します。
pub fn is_user_an_admin() -> bool {
    let mut handle: HANDLE = HANDLE::default();
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle).is_ok() {
            let mut elevation = TOKEN_ELEVATION::default();
            let mut size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
            let result = GetTokenInformation(
                handle,
                TokenElevation,
                Some(&mut elevation as *mut _ as *mut _),
                size,
                &mut size,
            );
            return result.is_ok() && elevation.TokenIsElevated != 0;
        }
    }
    false
}

/// 管理者権限（UAC昇格プロンプト経由）で自身を再起動し、現在のプロセスを終了します。
pub fn restart_as_admin() -> Result<(), Box<dyn std::error::Error>> {
    let exe_path = std::env::current_exe()?;
    
    let mut path_wide: Vec<u16> = OsString::from(exe_path).encode_wide().collect();
    path_wide.push(0);

    // "runas" を指定することで管理者権限での実行を要求します
    let mut verb_wide: Vec<u16> = OsString::from("runas").encode_wide().collect();
    verb_wide.push(0);

    unsafe {
        let res = ShellExecuteW(
            HWND::default(),
            windows::core::PCWSTR::from_raw(verb_wide.as_ptr()),
            windows::core::PCWSTR::from_raw(path_wide.as_ptr()),
            windows::core::PCWSTR::null(),
            windows::core::PCWSTR::null(),
            windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD(SW_SHOWNOACTIVATE.0 as i32),
        );
        
        // ShellExecuteW は成功すると 32 より大きい値を返します
        if (res.0 as isize) <= 32 {
            return Err("管理者権限でのプロセス起動に失敗しました。".into());
        }
    }

    // 昇格プロセスの起動に成功したため、現在のプロセスは終了します
    std::process::exit(0);
}

/// タスクスケジューラを用いて、管理者権限での自動起動を登録または削除します。
/// 
/// * `task_name` - タスクスケジューラに登録する一意のタスク名（例: "MyCoolApp_AutoStart"）
/// * `enable` - true の場合は登録、false の場合は削除を行います
pub fn sync_admin_startup(task_name: &str, enable: bool) -> Result<(), Box<dyn std::error::Error>> {
    let exe_path = std::env::current_exe()?;
    let path_str = exe_path.to_string_lossy();

    // schtasks コマンドを実行するヘルパー
    let run_schtasks = |args: &[&str]| -> Result<std::process::Output, std::io::Error> {
        std::process::Command::new("schtasks")
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
    };

    if enable {
        // 既存の同名タスクがあれば競合防止のため削除（エラーは無視）
        let _ = run_schtasks(&["/Delete", "/F", "/TN", task_name]);

        // タスクスケジューラに登録
        // 自動起動であることを判定するための引数 `--autostart` を付与
        let tr_val = format!("\"{}\" --autostart", path_str);
        let output = run_schtasks(&[
            "/Create",
            "/F",
            "/TN",
            task_name,
            "/TR",
            &tr_val,
            "/SC",
            "ONLOGON",
            "/RL",
            "HIGHEST",
        ])?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("タスクの作成に失敗しました: {}", stderr.trim()).into());
        }
    } else {
        // タスクの削除
        let output = run_schtasks(&["/Delete", "/F", "/TN", task_name])?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // すでにタスクが存在しない場合のエラーは正常系として許容
            if stderr.contains("見つかりません") || stderr.contains("cannot find") || stderr.contains("not found") {
                return Ok(());
            }
            return Err(format!("タスクの削除に失敗しました: {}", stderr.trim()).into());
        }
    }
    Ok(())
}

/// 自動起動が登録されているか、および登録されているパスが現在の実行ファイルパスと一致するか確認します。
#[derive(serde::Serialize, Clone)]
pub struct AutostartStatus {
    pub is_registered: bool,
    pub is_same_path: bool,
    pub registered_path: Option<String>,
}

pub fn check_autostart_registered(task_name: &str) -> Result<AutostartStatus, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let path_str = exe_path.to_string_lossy().to_string();

    let output = std::process::Command::new("schtasks")
        .args(&["/Query", "/TN", task_name, "/XML"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    let default_fail = AutostartStatus {
        is_registered: false,
        is_same_path: false,
        registered_path: None,
    };

    match output {
        Ok(out) => {
            if !out.status.success() {
                return Ok(default_fail);
            }
            
            // PowerShellを利用して、登録されているタスクの実行ファイルパスを正確に取得します
            let script = format!(
                "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
                 $task = Get-ScheduledTask -TaskName '{}' -ErrorAction SilentlyContinue; \
                 if ($task) {{ Write-Output $task.Actions[0].Execute }} else {{ Write-Output 'NONE' }}",
                task_name
            );
            
            let ps_output = std::process::Command::new("powershell")
                .args(&["-NoProfile", "-Command", &script])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| e.to_string())?;
                
            if ps_output.status.success() {
                let mut res = String::from_utf8_lossy(&ps_output.stdout).trim().to_string();
                if res == "NONE" || res.is_empty() {
                    return Ok(default_fail);
                }
                
                // 引用符を除去して比較
                res = res.replace("\"", "").replace("'", "");
                let is_same = res.eq_ignore_ascii_case(&path_str);
                
                return Ok(AutostartStatus {
                    is_registered: true,
                    is_same_path: is_same,
                    registered_path: Some(res),
                });
            }
            Ok(default_fail)
        }
        Err(_) => Ok(default_fail),
    }
}
```

---

## 4. アプリケーション側の自動起動時のロジック設計（UX考慮）

自動起動されたプロセスの振る舞いについて、UX向上のために以下の設計パターンが推奨されます。

```
                       [ アプリケーション起動 ]
                                  │
                  ┌───────────────┴───────────────┐
                  ▼                               ▼
       引数に --autostart あり            引数に --autostart なし
                  │                               │
        [ GUIウィンドウを表示しない ]       [ 通常通りGUIウィンドウを表示 ]
        [ システムトレイ等に常駐 ]
```

### 1. コマンドライン引数の解析
- アプリケーション起動時に引数をチェックします。
- 引数の中に `--autostart` が含まれている場合、ウィンドウを非表示状態 (`hide`) もしくは最小化状態で起動し、システムトレイ（タスクトレイ）アイコンのみを表示します。
- ユーザーが手動でショートカット等から起動した場合は、引数がないため、通常通りGUIウィンドウを前面に表示します。

### 2. 重複起動の防止 (単一インスタンス起動制限)
- タスクスケジューラによる自動起動と、ユーザーによる手動起動が重複して立ち上がらないよう、ミューテックス (Named Mutex) 等を利用して、アプリケーションプロセスが二重起動しないよう保護します。
- すでに起動している場合は、手動起動された側のプロセスから既存プロセスにメッセージを送り、ウィンドウを表示させて終了するなどの設計が望ましいです。
