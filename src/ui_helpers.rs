/// UI描画の共通ヘルパー関数群
/// ウィンドウ操作、デバイス選択コンボボックス、音量メーターなど

use eframe::egui;
use crate::config::Config;
use crate::constants::APP_NAME;

// ========================================================================
// ウィンドウ操作
// ========================================================================

/// MicSplitter のウィンドウハンドルを取得する
unsafe fn find_app_window() -> *mut core::ffi::c_void {
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;
    let window_name: Vec<u16> = format!("{}\0", APP_NAME).encode_utf16().collect();
    unsafe { FindWindowW(std::ptr::null(), window_name.as_ptr()) }
}

/// ウィンドウを非表示にする（トレイへの格納用）
pub fn hide_window() {
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
        let hwnd = find_app_window();
        if hwnd != std::ptr::null_mut() {
            ShowWindow(hwnd, SW_HIDE);
        }
    }
}

/// ウィンドウを表示してフォーカスを当てる（トレイからの復帰用）
pub fn show_and_focus_window() {
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_RESTORE, SW_SHOW, SetForegroundWindow};
        let hwnd = find_app_window();
        if hwnd != std::ptr::null_mut() {
            ShowWindow(hwnd, SW_RESTORE);
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
        }
    }
}

// ========================================================================
// デバイス選択コンボボックス
// ========================================================================

/// デバイス選択コンボボックスを描画する共通関数
///
/// # 引数
/// - `ui`: egui の UI コンテキスト
/// - `id`: コンボボックスの一意な識別子
/// - `label_text`: ラベル文字列
/// - `current_name`: 現在選択されているデバイス名（変更可能な参照）
/// - `device_list`: 選択肢となるデバイス名の一覧
/// - `device_info`: デバイス情報 (サンプルレート, ビット深度, OS音量) - None の場合は切断扱い
/// - `color_icon`: ラベル左側に表示するカラーアイコンの色（省略可能）
/// - `allow_unselected`: 未選択状態を許可するかどうか
///
/// # 戻り値
/// デバイス名が変更された場合に `true`
pub fn device_combo_box(
    ui: &mut egui::Ui,
    id: &str,
    label_text: &str,
    current_name: &mut String,
    device_list: &[String],
    device_info: &Option<(u32, String, f32)>,
    color_icon: Option<[u8; 3]>,
    allow_unselected: bool,
) -> bool {
    let mut changed = false;
    let is_unselected = allow_unselected && current_name == "(未選択)";
    let is_invalid = is_unselected || device_info.is_none();

    ui.horizontal(|ui| {
        // カラーアイコンの描画（指定された場合のみ）
        if let Some(c) = color_icon {
            ui.label(egui::RichText::new("●").color(egui::Color32::from_rgb(c[0], c[1], c[2])));
        }

        // ラベル
        let label = egui::RichText::new(label_text);
        ui.label(if is_invalid { label.color(egui::Color32::RED) } else { label });

        // コンボボックス
        let mut selected_text = egui::RichText::new(current_name.as_str());
        if is_invalid {
            selected_text = selected_text.color(egui::Color32::RED);
        }

        #[allow(deprecated)]
        egui::ComboBox::from_id_source(id)
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                let mut display_list = device_list.to_vec();
                if allow_unselected {
                    let unselected_str = "(未選択)".to_string();
                    if !display_list.contains(&unselected_str) {
                        display_list.insert(0, unselected_str);
                    }
                }
                if !display_list.contains(current_name) {
                    display_list.insert(0, current_name.clone());
                }
                for dev in &display_list {
                    if ui.selectable_value(current_name, dev.clone(), dev).changed() {
                        changed = true;
                    }
                }
            });

        // デバイス情報の表示
        if is_invalid {
            ui.label(egui::RichText::new("(切断または無効)").color(egui::Color32::RED));
        } else if let Some((sr, bd, vol)) = device_info {
            ui.label(egui::RichText::new(
                format!("({}Hz, {}, OS音量: {:.0}%)", sr, bd, vol * 100.0)
            ).color(egui::Color32::from_gray(150)));
        }
    });

    changed
}

/// 出力デバイス名から対応する仮想マイク（録音デバイス）の名前を検索する
pub fn find_corresponding_virtual_mic(output_name: &str, input_devices: &[String]) -> Option<String> {
    if output_name == "(未選択)" {
        return None;
    }
    
    // VB-Audio Cableの対応表に基づくプレフィックスの抽出
    let prefix = if output_name.contains("CABLE-A") {
        "CABLE-A"
    } else if output_name.contains("CABLE-B") {
        "CABLE-B"
    } else if output_name.contains("CABLE-C") {
        "CABLE-C"
    } else if output_name.contains("CABLE-D") {
        "CABLE-D"
    } else if output_name.contains("CABLE") {
        "CABLE"
    } else {
        return None; // 対応していないデバイス
    };

    let target_str = format!("{} Output", prefix);
    for device in input_devices {
        if device.contains(&target_str) {
            return Some(device.clone());
        }
    }
    None
}

/// フロー図用のカスタムボタン描画
pub fn routing_button_ui(
    ui: &mut egui::Ui,
    label: &str,
    is_enabled: bool,
    is_invalid: bool,
    is_interactive: bool,
    min_width: f32,
) -> egui::Response {
    let fill_color = if is_invalid {
        egui::Color32::from_rgb(180, 50, 50) // 赤（無効・未選択）
    } else if is_enabled {
        egui::Color32::from_rgb(0, 150, 0) // 緑（ON）
    } else {
        ui.style().visuals.widgets.inactive.bg_fill // デフォルト（OFF）
    };

    let text_color = if is_invalid {
        egui::Color32::from_rgb(255, 200, 200)
    } else if is_enabled {
        egui::Color32::WHITE
    } else {
        ui.style().visuals.text_color()
    };

    let button = egui::Button::new(egui::RichText::new(label).color(text_color))
        .fill(fill_color)
        .min_size(egui::vec2(min_width, 0.0));
    
    if !is_interactive || is_invalid {
        ui.add(button.sense(egui::Sense::hover()))
    } else {
        ui.add(button)
    }
}

// ========================================================================
// 音量メーター
// ========================================================================

/// 音量メーターの表示状態
#[derive(Default, Clone)]
pub struct VolumeMeterState {
    pub display_value: f32,
    pub peak_value: f32,
    pub peak_time: f64,
}

/// 音量メーターを描画する
///
/// RMS値に応じて3段階のゾーン（安全/警告/危険）をカスタムバーで表示し、
/// ピークホールドマーカーを描画する。ミュート時はグレーアウトと「MUTED」表示。
pub fn volume_meter_ui(
    ui: &mut egui::Ui,
    rms: f32,
    state: &mut VolumeMeterState,
    is_enabled: bool,
    config: &Config,
) {
    use egui::*;
    let desired_size = vec2(config.meter_width, config.meter_height);
    let (rect, _response) = ui.allocate_exact_size(desired_size, Sense::hover());

    if !ui.is_rect_visible(rect) {
        return;
    }

    let painter = ui.painter();

    // 背景を描画（暗いグレー）
    painter.rect_filled(rect, 2.0, Color32::from_gray(30));

    let now = ui.input(|i| i.time);
    let dt = ui.input(|i| i.stable_dt).min(0.1) as f32;

    // RMS を 0.0〜1.0 の範囲に正規化
    let raw_val = (rms * config.meter_rms_scale).clamp(0.0, 1.0);

    // 上昇は瞬時、下降は滑らかに（減衰）
    if raw_val > state.display_value {
        state.display_value = raw_val;
    } else {
        state.display_value = (state.display_value - config.meter_decay_speed * dt).max(raw_val);
    }

    // ピークホールドの更新
    if raw_val >= state.peak_value {
        state.peak_value = raw_val;
        state.peak_time = now;
    } else if now - state.peak_time > config.meter_peak_hold_secs {
        state.peak_value = (state.peak_value - config.meter_peak_decay_speed * dt).max(raw_val);
    }

    let width = rect.width();
    let safe_zone = config.meter_safe_zone;
    let warn_zone = config.meter_warn_zone;
    let c_safe = config.meter_color_safe;
    let c_warn = config.meter_color_warn;
    let c_danger = config.meter_color_danger;

    if is_enabled {
        let bar_width = width * state.display_value;
        let green_width = width * safe_zone;
        let yellow_width = width * (warn_zone - safe_zone);

        let mut current_x = rect.min.x;

        // 安全域 (0 - safe_zone) 緑色
        if state.display_value > 0.0 {
            let w = bar_width.min(green_width);
            let r = Rect::from_min_size(pos2(current_x, rect.min.y), vec2(w, rect.height()));
            painter.rect_filled(r, 0.0, Color32::from_rgb(c_safe[0], c_safe[1], c_safe[2]));
            current_x += w;
        }

        // 警告域 (safe_zone - warn_zone) 黄色
        if state.display_value > safe_zone {
            let w = (bar_width - green_width).min(yellow_width);
            let r = Rect::from_min_size(pos2(current_x, rect.min.y), vec2(w, rect.height()));
            painter.rect_filled(r, 0.0, Color32::from_rgb(c_warn[0], c_warn[1], c_warn[2]));
            current_x += w;
        }

        // 危険域 (warn_zone - 1.0) 赤色
        if state.display_value > warn_zone {
            let w = bar_width - green_width - yellow_width;
            let r = Rect::from_min_size(pos2(current_x, rect.min.y), vec2(w, rect.height()));
            painter.rect_filled(r, 0.0, Color32::from_rgb(c_danger[0], c_danger[1], c_danger[2]));
        }

        // ピークホールドマーカーの描画
        if state.peak_value > 0.0 {
            let peak_x = rect.min.x + width * state.peak_value;
            let peak_color = if state.peak_value > warn_zone {
                Color32::from_rgb(255, 100, 100)
            } else if state.peak_value > safe_zone {
                Color32::from_rgb(255, 255, 100)
            } else {
                Color32::from_rgb(100, 255, 100)
            };
            painter.line_segment(
                [pos2(peak_x, rect.min.y), pos2(peak_x, rect.max.y)],
                Stroke::new(2.0, peak_color),
            );
        }
    } else {
        // ミュート時の表示（グレーアウト）
        let bar_width = width * state.display_value;
        if bar_width > 0.0 {
            let bar_rect = Rect::from_min_size(rect.min, vec2(bar_width, rect.height()));
            painter.rect_filled(bar_rect, 0.0, Color32::from_gray(80));
        }

        // ミュートの明示（文字）
        let center = rect.center();
        painter.text(
            center,
            Align2::CENTER_CENTER,
            "MUTED",
            FontId::proportional(12.0),
            Color32::from_gray(200),
        );
    }

    // 枠線の描画
    painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_gray(60)));
}
