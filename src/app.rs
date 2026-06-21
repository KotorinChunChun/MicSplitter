use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use cpal::Stream;
use crate::config::{Config, save_config};

pub struct MicSplitterApp {
    pub config: Config,
    pub out1_enabled: Arc<AtomicBool>,
    pub out2_enabled: Arc<AtomicBool>,
    pub mon_enabled: Arc<AtomicBool>,
    // アプリ生存中ストリームを生かしておくため保持
    pub _input_stream: Stream,
    pub _stream1: Stream,
    pub _stream2: Stream,
    pub _stream_mon: Stream,
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
        eframe::egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("MicSplitter");
            
            ui.separator();
            ui.label(format!("入力デバイス: {}", self.config.input_device_name));
            ui.separator();

            let mut config_changed = false;

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
}
