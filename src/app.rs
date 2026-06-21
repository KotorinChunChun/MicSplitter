use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use cpal::Stream;
use crate::config::{Config, save_config};

pub struct MicSplitterApp {
    pub config: Config,
    pub in_enabled: Arc<AtomicBool>,
    pub out1_enabled: Arc<AtomicBool>,
    pub out2_enabled: Arc<AtomicBool>,
    pub mon_enabled: Arc<AtomicBool>,
    pub _input_stream: Stream,
    pub _stream1: Stream,
    pub _stream2: Stream,
    pub _stream_mon: Stream,
    pub is_hidden: bool,
    pub should_show: Arc<AtomicBool>,
}

pub fn setup_custom_fonts(ctx: &eframe::egui::Context) {
    let mut fonts = eframe::egui::FontDefinitions::default();

    // Windows標準のメイリオを読み込む。失敗した場合はMSゴシックにフォールバック
    let font_data = std::fs::read("C:\\Windows\\Fonts\\meiryo.ttc").unwrap_or_else(|_| {
        std::fs::read("C:\\Windows\\Fonts\\msgothic.ttc").expect("日本語フォントが見つかりません")
    });

    fonts.font_data.insert(
        "my_font".to_owned(),
        eframe::egui::FontData::from_owned(font_data),
    );

    // すべてのフォントファミリの最優先にこのフォントを設定
    fonts.families.get_mut(&eframe::egui::FontFamily::Proportional).unwrap().insert(0, "my_font".to_owned());
    fonts.families.get_mut(&eframe::egui::FontFamily::Monospace).unwrap().insert(0, "my_font".to_owned());

    ctx.set_fonts(fonts);
}

impl eframe::App for MicSplitterApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        if self.should_show.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            self.is_hidden = false;
        }

        // 最小化されたら非表示にする
        if ctx.input(|i| i.viewport().minimized.unwrap_or(false)) {
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Minimized(false)); // 最小化状態を解除しておく
            unsafe {
                use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, ShowWindow, SW_HIDE};
                let window_name: Vec<u16> = "MicSplitter\0".encode_utf16().collect();
                let hwnd = FindWindowW(std::ptr::null(), window_name.as_ptr());
                if hwnd != std::ptr::null_mut() {
                    ShowWindow(hwnd, SW_HIDE);
                }
            }
            self.is_hidden = true;
        }

        // ウィンドウのクローズイベントをキャンセルして非表示にする処理
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::CancelClose);
            unsafe {
                use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, ShowWindow, SW_HIDE};
                let window_name: Vec<u16> = "MicSplitter\0".encode_utf16().collect();
                let hwnd = FindWindowW(std::ptr::null(), window_name.as_ptr());
                if hwnd != std::ptr::null_mut() {
                    ShowWindow(hwnd, SW_HIDE);
                }
            }
            self.is_hidden = true;
        }

        // バックグラウンド等により外部で AtomicBool が変更された場合、設定と同期して保存
        let mut changed = false;
        let current_in = self.in_enabled.load(Ordering::Relaxed);
        if self.config.input_enabled != current_in {
            self.config.input_enabled = current_in;
            changed = true;
        }
        let current_out1 = self.out1_enabled.load(Ordering::Relaxed);
        if self.config.output_device_1_enabled != current_out1 {
            self.config.output_device_1_enabled = current_out1;
            changed = true;
        }
        let current_out2 = self.out2_enabled.load(Ordering::Relaxed);
        if self.config.output_device_2_enabled != current_out2 {
            self.config.output_device_2_enabled = current_out2;
            changed = true;
        }
        let current_mon = self.mon_enabled.load(Ordering::Relaxed);
        if self.config.monitor_enabled != current_mon {
            self.config.monitor_enabled = current_mon;
            changed = true;
        }
        if changed {
            save_config("config.json", &self.config);
            ctx.request_repaint(); // 表示の即時反映
        }

        if !self.is_hidden {
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("MicSplitter");
            
            let mut config_changed = false;

            ui.separator();
            ui.horizontal(|ui| {
                ui.label(format!("入力デバイス: {}", self.config.input_device_name));
                if ui.checkbox(&mut self.config.input_enabled, "ON (ミュート解除)").changed() {
                    self.in_enabled.store(self.config.input_enabled, Ordering::Relaxed);
                    config_changed = true;
                }
            });
            ui.separator();

            // Output 1
            ui.horizontal(|ui| {
                ui.label(format!("出力1 (仮想マイクA): {}", self.config.output_device_1_name));
                if ui.checkbox(&mut self.config.output_device_1_enabled, "ON (ミュート解除)").changed() {
                    self.out1_enabled.store(self.config.output_device_1_enabled, Ordering::Relaxed);
                    config_changed = true;
                }
            });

            // Output 2
            ui.horizontal(|ui| {
                ui.label(format!("出力2 (仮想マイクB): {}", self.config.output_device_2_name));
                if ui.checkbox(&mut self.config.output_device_2_enabled, "ON (ミュート解除)").changed() {
                    self.out2_enabled.store(self.config.output_device_2_enabled, Ordering::Relaxed);
                    config_changed = true;
                }
            });

            ui.separator();

            // Monitor
            ui.horizontal(|ui| {
                ui.label(format!("モニター出力: {}", self.config.monitor_device_name));
                if ui.checkbox(&mut self.config.monitor_enabled, "モニターON").changed() {
                    self.mon_enabled.store(self.config.monitor_enabled, Ordering::Relaxed);
                    config_changed = true;
                }
            });

            if config_changed {
                save_config("config.json", &self.config);
            }
        });
        }
        
        // バックグラウンドでもイベントを受信し続けるため、再描画をリクエスト
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
}
