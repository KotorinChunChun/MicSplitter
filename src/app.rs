use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use cpal::Stream;
use crate::config::{Config, save_config};

#[derive(Default, Clone)]
pub struct VolumeMeterState {
    pub display_value: f32,
    pub peak_value: f32,
    pub peak_time: f64,
}

pub struct MicSplitterApp {
    pub config: Config,
    pub in_enabled: Arc<AtomicBool>,
    pub out1_enabled: Arc<AtomicBool>,
    pub out2_enabled: Arc<AtomicBool>,
    pub mon_enabled: Arc<AtomicBool>,
    pub is_toggle_mode: Arc<AtomicBool>,
    pub mon_volume_bits: Arc<std::sync::atomic::AtomicU32>,
    pub in_rms_bits: Arc<std::sync::atomic::AtomicU32>,
    pub out1_rms_bits: Arc<std::sync::atomic::AtomicU32>,
    pub out2_rms_bits: Arc<std::sync::atomic::AtomicU32>,
    pub mon_rms_bits: Arc<std::sync::atomic::AtomicU32>,
    pub _input_stream: Option<Stream>,
    pub _stream1: Option<Stream>,
    pub _stream2: Option<Stream>,
    pub _stream_mon: Option<Stream>,
    pub is_hidden: bool,
    pub should_show: Arc<AtomicBool>,
    pub input_devices: Vec<String>,
    pub output_devices: Vec<String>,
    pub stream_error: Option<String>,
    pub in_meter_state: VolumeMeterState,
    pub out1_meter_state: VolumeMeterState,
    pub out2_meter_state: VolumeMeterState,
    pub mon_meter_state: VolumeMeterState,
    pub in_device_info: Option<(u32, String, f32)>,
    pub out1_device_info: Option<(u32, String, f32)>,
    pub out2_device_info: Option<(u32, String, f32)>,
    pub mon_device_info: Option<(u32, String, f32)>,
    pub device_refresh_rx: Option<std::sync::mpsc::Receiver<()>>,
    pub window_pos: std::sync::Arc<std::sync::Mutex<Option<(f32, f32)>>>,
}

impl MicSplitterApp {
    fn rebuild_streams(&mut self) {
        self._input_stream = None;
        self._stream1 = None;
        self._stream2 = None;
        self._stream_mon = None;

        let host = cpal::default_host();
        self.in_device_info = crate::audio::get_device_info(&host, &self.config.input_device_name, true);
        self.out1_device_info = crate::audio::get_device_info(&host, &self.config.output_device_1_name, false);
        self.out2_device_info = crate::audio::get_device_info(&host, &self.config.output_device_2_name, false);
        self.mon_device_info = crate::audio::get_device_info(&host, &self.config.monitor_device_name, false);

        match crate::audio::build_all_streams(
            &self.config,
            self.in_enabled.clone(),
            self.out1_enabled.clone(),
            self.out2_enabled.clone(),
            self.mon_enabled.clone(),
            self.mon_volume_bits.clone(),
            self.in_rms_bits.clone(),
            self.out1_rms_bits.clone(),
            self.out2_rms_bits.clone(),
            self.mon_rms_bits.clone(),
        ) {
            Ok(streams) => {
                self._input_stream = Some(streams.input_stream);
                self._stream1 = streams.stream1;
                self._stream2 = streams.stream2;
                self._stream_mon = streams.stream_mon;
                self.stream_error = None;
            }
            Err(e) => {
                self.stream_error = Some(e);
            }
        }
    }
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
        if let Some(rx) = &self.device_refresh_rx {
            if rx.try_recv().is_ok() {
                self.input_devices = crate::audio::get_input_devices();
                self.output_devices = crate::audio::get_output_devices();
                let host = cpal::default_host();
                self.in_device_info = crate::audio::get_device_info(&host, &self.config.input_device_name, true);
                self.out1_device_info = crate::audio::get_device_info(&host, &self.config.output_device_1_name, false);
                self.out2_device_info = crate::audio::get_device_info(&host, &self.config.output_device_2_name, false);
                self.mon_device_info = crate::audio::get_device_info(&host, &self.config.monitor_device_name, false);
                self.rebuild_streams();
                ctx.request_repaint();
            }
        }

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
            if let Some((x, y)) = *self.window_pos.lock().unwrap() {
                self.config.window_pos_x = Some(x);
                self.config.window_pos_y = Some(y);
            }
            save_config("config.json", &self.config);
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
            if let Some((x, y)) = *self.window_pos.lock().unwrap() {
                self.config.window_pos_x = Some(x);
                self.config.window_pos_y = Some(y);
            }
            save_config("config.json", &self.config);
        }

        // アニメーションを滑らかにするため、ウィンドウ表示中は常に再描画を要求し、位置を記録する
        if !self.is_hidden {
            if let Some(rect) = ctx.input(|i| i.viewport().outer_rect) {
                if rect.min.x > -10000.0 && rect.min.y > -10000.0 {
                    *self.window_pos.lock().unwrap() = Some((rect.min.x, rect.min.y));
                }
            }
            ctx.request_repaint();
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
            eframe::egui::CollapsingHeader::new("ショートカットキー").default_open(true).show(ui, |ui| {
                ui.label("入力ミュート: Ctrl + Alt + Win + F7");
                ui.label("モニターミュート: Ctrl + Alt + Win + F8");
                ui.label("出力1 (仮想マイクA) トグル: Ctrl + Alt + Win + F9");
                ui.label("出力2 (仮想マイクB) トグル: Ctrl + Alt + Win + F10");
                ui.label("出力1・2 スワップ: Ctrl + Alt + Win + F11");
            });

            ui.separator();

            eframe::egui::CollapsingHeader::new("デバイス設定").default_open(true).show(ui, |ui| {
                let mut changed = false;

                if ui.button("デバイスリストを更新").clicked() {
                    self.input_devices = crate::audio::get_input_devices();
                    self.output_devices = crate::audio::get_output_devices();
                    let host = cpal::default_host();
                    self.in_device_info = crate::audio::get_device_info(&host, &self.config.input_device_name, true);
                    self.out1_device_info = crate::audio::get_device_info(&host, &self.config.output_device_1_name, false);
                    self.out2_device_info = crate::audio::get_device_info(&host, &self.config.output_device_2_name, false);
                    self.mon_device_info = crate::audio::get_device_info(&host, &self.config.monitor_device_name, false);
                    self.rebuild_streams();
                }

                ui.horizontal(|ui| {
                    let is_invalid = self.in_device_info.is_none();
                    let label = eframe::egui::RichText::new("入力デバイス:");
                    ui.label(if is_invalid { label.color(eframe::egui::Color32::RED) } else { label });
                    
                    let mut selected_text = eframe::egui::RichText::new(&self.config.input_device_name);
                    if is_invalid { selected_text = selected_text.color(eframe::egui::Color32::RED); }

                    eframe::egui::ComboBox::from_id_source("in_dev")
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            let mut display_list = self.input_devices.clone();
                            if !display_list.contains(&self.config.input_device_name) {
                                display_list.insert(0, self.config.input_device_name.clone());
                            }
                            for dev in &display_list {
                                if ui.selectable_value(&mut self.config.input_device_name, dev.clone(), dev).changed() {
                                    changed = true;
                                }
                            }
                        });
                    
                    if is_invalid {
                        ui.label(eframe::egui::RichText::new("(切断または無効)").color(eframe::egui::Color32::RED));
                    } else if let Some((sr, bd, vol)) = &self.in_device_info {
                        ui.label(eframe::egui::RichText::new(format!("({}Hz, {}, OS音量: {:.0}%)", sr, bd, vol * 100.0)).color(eframe::egui::Color32::from_gray(150)));
                    }
                });

                ui.horizontal(|ui| {
                    let is_invalid = self.out1_device_info.is_none();
                    let label = eframe::egui::RichText::new("仮想出力1:");
                    let c = self.config.icon_color_out1_on;
                    ui.label(eframe::egui::RichText::new("●").color(eframe::egui::Color32::from_rgb(c[0], c[1], c[2])));
                    ui.label(if is_invalid { label.color(eframe::egui::Color32::RED) } else { label });

                    let mut selected_text = eframe::egui::RichText::new(&self.config.output_device_1_name);
                    if is_invalid { selected_text = selected_text.color(eframe::egui::Color32::RED); }

                    eframe::egui::ComboBox::from_id_source("out1_dev")
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            let mut display_list = self.output_devices.clone();
                            if !display_list.contains(&self.config.output_device_1_name) {
                                display_list.insert(0, self.config.output_device_1_name.clone());
                            }
                            for dev in &display_list {
                                if ui.selectable_value(&mut self.config.output_device_1_name, dev.clone(), dev).changed() {
                                    changed = true;
                                }
                            }
                        });
                    
                    if is_invalid {
                        ui.label(eframe::egui::RichText::new("(切断または無効)").color(eframe::egui::Color32::RED));
                    } else if let Some((sr, bd, vol)) = &self.out1_device_info {
                        ui.label(eframe::egui::RichText::new(format!("({}Hz, {}, OS音量: {:.0}%)", sr, bd, vol * 100.0)).color(eframe::egui::Color32::from_gray(150)));
                    }
                });

                ui.horizontal(|ui| {
                    let is_invalid = self.out2_device_info.is_none();
                    let label = eframe::egui::RichText::new("仮想出力2:");
                    let c = self.config.icon_color_out2_on;
                    ui.label(eframe::egui::RichText::new("●").color(eframe::egui::Color32::from_rgb(c[0], c[1], c[2])));
                    ui.label(if is_invalid { label.color(eframe::egui::Color32::RED) } else { label });

                    let mut selected_text = eframe::egui::RichText::new(&self.config.output_device_2_name);
                    if is_invalid { selected_text = selected_text.color(eframe::egui::Color32::RED); }

                    eframe::egui::ComboBox::from_id_source("out2_dev")
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            let mut display_list = self.output_devices.clone();
                            if !display_list.contains(&self.config.output_device_2_name) {
                                display_list.insert(0, self.config.output_device_2_name.clone());
                            }
                            for dev in &display_list {
                                if ui.selectable_value(&mut self.config.output_device_2_name, dev.clone(), dev).changed() {
                                    changed = true;
                                }
                            }
                        });
                    
                    if is_invalid {
                        ui.label(eframe::egui::RichText::new("(切断または無効)").color(eframe::egui::Color32::RED));
                    } else if let Some((sr, bd, vol)) = &self.out2_device_info {
                        ui.label(eframe::egui::RichText::new(format!("({}Hz, {}, OS音量: {:.0}%)", sr, bd, vol * 100.0)).color(eframe::egui::Color32::from_gray(150)));
                    }
                });

                ui.horizontal(|ui| {
                    let is_invalid = self.mon_device_info.is_none();
                    let label = eframe::egui::RichText::new("モニター出力:");
                    ui.label(if is_invalid { label.color(eframe::egui::Color32::RED) } else { label });

                    let mut selected_text = eframe::egui::RichText::new(&self.config.monitor_device_name);
                    if is_invalid { selected_text = selected_text.color(eframe::egui::Color32::RED); }

                    eframe::egui::ComboBox::from_id_source("mon_dev")
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            let mut display_list = self.output_devices.clone();
                            if !display_list.contains(&self.config.monitor_device_name) {
                                display_list.insert(0, self.config.monitor_device_name.clone());
                            }
                            for dev in &display_list {
                                if ui.selectable_value(&mut self.config.monitor_device_name, dev.clone(), dev).changed() {
                                    changed = true;
                                }
                            }
                        });
                    
                    if is_invalid {
                        ui.label(eframe::egui::RichText::new("(切断または無効)").color(eframe::egui::Color32::RED));
                    } else if let Some((sr, bd, vol)) = &self.mon_device_info {
                        ui.label(eframe::egui::RichText::new(format!("({}Hz, {}, OS音量: {:.0}%)", sr, bd, vol * 100.0)).color(eframe::egui::Color32::from_gray(150)));
                    }
                });

                if changed {
                    save_config("config.json", &self.config);
                    self.rebuild_streams();
                }
            });

            if let Some(err) = &self.stream_error {
                ui.separator();
                ui.label(eframe::egui::RichText::new(format!("エラー: {}", err)).color(eframe::egui::Color32::RED));
            }

            ui.separator();

            eframe::egui::CollapsingHeader::new("ボリュームとミュート設定").default_open(true).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("切り替えモード:");
                    if ui.radio_value(&mut self.config.switching_mode, "toggle".to_string(), "トグル（排他）").changed() {
                        self.is_toggle_mode.store(true, Ordering::Relaxed);
                        config_changed = true;
                    }
                    if ui.radio_value(&mut self.config.switching_mode, "individual".to_string(), "個別ON/OFF").changed() {
                        self.is_toggle_mode.store(false, Ordering::Relaxed);
                        config_changed = true;
                    }
                });

                ui.separator();

                // Input
                ui.horizontal(|ui| {
                    let is_invalid = self.in_device_info.is_none();
                    let label = eframe::egui::RichText::new(format!("入力 (マイク): {}", self.config.input_device_name));
                    ui.label(if is_invalid { label.color(eframe::egui::Color32::RED) } else { label });
                    let rms = f32::from_bits(self.in_rms_bits.load(Ordering::Relaxed));
                    volume_meter_ui(ui, rms, &mut self.in_meter_state, self.config.input_enabled);
                    if ui.checkbox(&mut self.config.input_enabled, "ON (ミュート解除)").changed() {
                        self.in_enabled.store(self.config.input_enabled, Ordering::Relaxed);
                        config_changed = true;
                    }
                });

                // Output 1
                ui.horizontal(|ui| {
                    let is_invalid = self.out1_device_info.is_none();
                    let label = eframe::egui::RichText::new(format!("出力1 (仮想マイクA): {}", self.config.output_device_1_name));
                    let c = self.config.icon_color_out1_on;
                    ui.label(eframe::egui::RichText::new("●").color(eframe::egui::Color32::from_rgb(c[0], c[1], c[2])));
                    ui.label(if is_invalid { label.color(eframe::egui::Color32::RED) } else { label });
                    let rms = f32::from_bits(self.out1_rms_bits.load(Ordering::Relaxed));
                    volume_meter_ui(ui, rms, &mut self.out1_meter_state, self.config.output_device_1_enabled);
                    if ui.checkbox(&mut self.config.output_device_1_enabled, "ON (ミュート解除)").changed() {
                        if self.config.switching_mode == "toggle" && self.config.output_device_1_enabled {
                            self.config.output_device_2_enabled = false;
                            self.out2_enabled.store(false, Ordering::Relaxed);
                        }
                        self.out1_enabled.store(self.config.output_device_1_enabled, Ordering::Relaxed);
                        config_changed = true;
                    }
                });

                // Output 2
                ui.horizontal(|ui| {
                    let is_invalid = self.out2_device_info.is_none();
                    let label = eframe::egui::RichText::new(format!("出力2 (仮想マイクB): {}", self.config.output_device_2_name));
                    let c = self.config.icon_color_out2_on;
                    ui.label(eframe::egui::RichText::new("●").color(eframe::egui::Color32::from_rgb(c[0], c[1], c[2])));
                    ui.label(if is_invalid { label.color(eframe::egui::Color32::RED) } else { label });
                    let rms = f32::from_bits(self.out2_rms_bits.load(Ordering::Relaxed));
                    volume_meter_ui(ui, rms, &mut self.out2_meter_state, self.config.output_device_2_enabled);
                    if ui.checkbox(&mut self.config.output_device_2_enabled, "ON (ミュート解除)").changed() {
                        if self.config.switching_mode == "toggle" && self.config.output_device_2_enabled {
                            self.config.output_device_1_enabled = false;
                            self.out1_enabled.store(false, Ordering::Relaxed);
                        }
                        self.out2_enabled.store(self.config.output_device_2_enabled, Ordering::Relaxed);
                        config_changed = true;
                    }
                });

                // Monitor
                ui.horizontal(|ui| {
                    let is_invalid = self.mon_device_info.is_none();
                    let label = eframe::egui::RichText::new(format!("モニター出力: {}", self.config.monitor_device_name));
                    ui.label(if is_invalid { label.color(eframe::egui::Color32::RED) } else { label });
                    let rms = f32::from_bits(self.mon_rms_bits.load(Ordering::Relaxed));
                    volume_meter_ui(ui, rms, &mut self.mon_meter_state, self.config.monitor_enabled);
                    if ui.checkbox(&mut self.config.monitor_enabled, "モニターON").changed() {
                        self.mon_enabled.store(self.config.monitor_enabled, Ordering::Relaxed);
                        config_changed = true;
                    }
                });
                if ui.add(eframe::egui::Slider::new(&mut self.config.monitor_volume, 0.0..=1.0).text("モニター音量")).changed() {
                    self.mon_volume_bits.store(self.config.monitor_volume.to_bits(), Ordering::Relaxed);
                    config_changed = true;
                }
            });

            ui.separator();
            if ui.button("サウンド設定を開く (コントロールパネル)").clicked() {
                let _ = std::process::Command::new("control.exe").arg("mmsys.cpl").spawn();
            }

            if config_changed {
                save_config("config.json", &self.config);
            }
        });
        }
        
        // バックグラウンドでもイベントを受信し続けるため、再描画をリクエスト
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
}

fn volume_meter_ui(ui: &mut eframe::egui::Ui, rms: f32, state: &mut VolumeMeterState, is_enabled: bool) {
    use eframe::egui::*;
    let desired_size = vec2(150.0, 16.0);
    let (rect, _response) = ui.allocate_exact_size(desired_size, Sense::hover());
    
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        
        // 背景を描画 (暗いグレー)
        painter.rect_filled(rect, 2.0, Color32::from_gray(30));
        
        let now = ui.input(|i| i.time);
        let dt = ui.input(|i| i.stable_dt).min(0.1) as f32;
        
        // RMSを 0.0〜1.0 の範囲に正規化 (適度にスケーリング)
        let raw_val = (rms * 5.0).clamp(0.0, 1.0);
        
        // 上昇は瞬時、下降は滑らかに (減衰)
        if raw_val > state.display_value {
            state.display_value = raw_val;
        } else {
            // 毎秒 1.0 の速度で落下
            state.display_value = (state.display_value - 1.0 * dt).max(raw_val);
        }
        
        // ピークホールドの更新
        if raw_val >= state.peak_value {
            state.peak_value = raw_val;
            state.peak_time = now;
        } else if now - state.peak_time > 1.5 {
            // 1.5秒経過したらピークをゆっくり下げる
            state.peak_value = (state.peak_value - 0.5 * dt).max(raw_val);
        }
        
        let width = rect.width();
        
        if is_enabled {
            let bar_width = width * state.display_value;
            
            let green_width = width * 0.7;
            let yellow_width = width * 0.2;
            
            let mut current_x = rect.min.x;
            
            // 安全域 (0 - 70%) 緑色
            if state.display_value > 0.0 {
                let w = bar_width.min(green_width);
                let r = Rect::from_min_size(pos2(current_x, rect.min.y), vec2(w, rect.height()));
                painter.rect_filled(r, 0.0, Color32::from_rgb(40, 200, 40));
                current_x += w;
            }
            
            // 警告域 (70 - 90%) 黄色
            if state.display_value > 0.7 {
                let w = (bar_width - green_width).min(yellow_width);
                let r = Rect::from_min_size(pos2(current_x, rect.min.y), vec2(w, rect.height()));
                painter.rect_filled(r, 0.0, Color32::from_rgb(220, 200, 40));
                current_x += w;
            }
            
            // 危険域 (90 - 100%) 赤色
            if state.display_value > 0.9 {
                let w = bar_width - green_width - yellow_width;
                let r = Rect::from_min_size(pos2(current_x, rect.min.y), vec2(w, rect.height()));
                painter.rect_filled(r, 0.0, Color32::from_rgb(220, 40, 40));
            }
            
            // ピークホールドマーカーの描画
            if state.peak_value > 0.0 {
                let peak_x = rect.min.x + width * state.peak_value;
                let peak_color = if state.peak_value > 0.9 {
                    Color32::from_rgb(255, 100, 100)
                } else if state.peak_value > 0.7 {
                    Color32::from_rgb(255, 255, 100)
                } else {
                    Color32::from_rgb(100, 255, 100)
                };
                painter.line_segment(
                    [pos2(peak_x, rect.min.y), pos2(peak_x, rect.max.y)],
                    Stroke::new(2.0, peak_color)
                );
            }
        } else {
            // ミュート時の表示 (グレーアウト)
            let bar_width = width * state.display_value;
            if bar_width > 0.0 {
                let bar_rect = Rect::from_min_size(rect.min, vec2(bar_width, rect.height()));
                painter.rect_filled(bar_rect, 0.0, Color32::from_gray(80));
            }
            
            // ミュートの明示 (文字)
            let center = rect.center();
            painter.text(
                center,
                Align2::CENTER_CENTER,
                "MUTED",
                FontId::proportional(12.0),
                Color32::from_gray(200)
            );
        }
        
        // 枠線の描画
        painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_gray(60)));
    }
}
