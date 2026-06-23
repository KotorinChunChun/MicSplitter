/// タスクトレイのアイコン生成・管理
/// 出力状態に応じた色分け、入力ミュート時の×印描画を行う

use tray_icon::{TrayIcon, TrayIconBuilder, menu::{Menu, MenuItem, PredefinedMenuItem}};
use tray_icon::Icon;
use crate::constants::APP_NAME;

#[allow(dead_code)]
pub struct TraySetup {
    pub tray_icon: TrayIcon,
    pub show_id: String,
    pub quit_id: String,
}

/// 出力状態と入力ミュート状態に応じたトレイアイコンを生成する
///
/// - 両方ON: 左上を出力1色、右下を出力2色で斜め分割
/// - 片方ON: 対応する出力色で塗りつぶし
/// - 両方OFF: グレーで塗りつぶし
/// - 入力ミュート時: 赤い×印を上に重ねて描画
pub fn generate_icon(in_enabled: bool, out1: bool, out2: bool, cfg: &crate::config::Config) -> Icon {
    let size = cfg.tray_icon_size;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);

    for y in 0..size {
        for x in 0..size {
            let [mut r, mut g, mut b] = match (out1, out2) {
                (true, true) => {
                    // 対角線で斜め分割: 左上が出力1、右下が出力2
                    if x + y < size {
                        cfg.icon_color_out1_on
                    } else {
                        cfg.icon_color_out2_on
                    }
                },
                (true, false) => cfg.icon_color_out1_on,
                (false, true) => cfg.icon_color_out2_on,
                (false, false) => cfg.icon_color_both_off,
            };

            // 入力ミュート時の×印（赤色）
            if !in_enabled {
                let margin = size / 5;        // アイコン内のマージン
                let thickness = size / 16 + 1; // 線の太さ
                let is_cross = (x as i32 - y as i32).abs() <= thickness as i32
                    || (x as i32 + y as i32 - (size as i32 - 1)).abs() <= thickness as i32;
                let in_bounds = x > margin && x < size - margin && y > margin && y < size - margin;
                if is_cross && in_bounds {
                    r = 255;
                    g = 0;
                    b = 0;
                }
            }

            rgba.push(r);
            rgba.push(g);
            rgba.push(b);
            rgba.push(255); // A
        }
    }
    Icon::from_rgba(rgba, size, size).unwrap()
}

/// トレイアイコンを現在の状態に合わせて更新する
pub fn update_tray_icon(tray_icon: &TrayIcon, in_enabled: bool, out1: bool, out2: bool, cfg: &crate::config::Config) {
    let icon = generate_icon(in_enabled, out1, out2, cfg);
    let _ = tray_icon.set_icon(Some(icon));
}

/// トレイアイコンとメニューを初期化する
pub fn create_tray_icon(initial_in: bool, initial_out1: bool, initial_out2: bool, cfg: &crate::config::Config) -> Result<TraySetup, Box<dyn std::error::Error>> {
    let tray_menu = Menu::new();
    let show_item = MenuItem::with_id("show", "Show", true, None);
    let quit_item = MenuItem::with_id("quit", "Quit", true, None);

    let show_id = show_item.id().clone().0;
    let quit_id = quit_item.id().clone().0;

    tray_menu.append_items(&[
        &show_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ])?;

    let icon = generate_icon(initial_in, initial_out1, initial_out2, cfg);

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_menu_on_left_click(false)
        .with_tooltip(APP_NAME)
        .with_icon(icon)
        .build()?;

    Ok(TraySetup {
        tray_icon,
        show_id,
        quit_id,
    })
}
