use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use cpal::Stream;
use crate::config::{Config, save_config};
use crate::constants::CONFIG_FILE;
use crate::ui_helpers::{self, VolumeMeterState};

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
    pub window_pos: Arc<std::sync::Mutex<Option<(f32, f32)>>>,
    pub window_size: Arc<std::sync::Mutex<Option<(f32, f32)>>>,
    pub waiting_for_key: WaitingForKey,
    pub hotkey_initial_keys: Option<[bool; 256]>,
    pub is_ptt_mode: Arc<AtomicBool>,
    pub ptt_out1_vk: Arc<std::sync::atomic::AtomicU32>,
    pub ptt_out2_vk: Arc<std::sync::atomic::AtomicU32>,
    pub in_hotkey_vk: Arc<std::sync::atomic::AtomicU32>,
    pub mon_hotkey_vk: Arc<std::sync::atomic::AtomicU32>,
    pub out1_hotkey_vk: Arc<std::sync::atomic::AtomicU32>,
    pub out2_hotkey_vk: Arc<std::sync::atomic::AtomicU32>,
}

#[derive(PartialEq)]
pub enum WaitingForKey {
    None,
    InToggle,
    MonToggle,
    Out1Toggle,
    Out2Toggle,
    Ptt1,
    Ptt2,
}

impl MicSplitterApp {
    /// オーディオストリームを再構築する
    fn rebuild_streams(&mut self) {
        self._input_stream = None;
        self._stream1 = None;
        self._stream2 = None;
        self._stream_mon = None;

        self.refresh_device_info();

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

    /// 全デバイスの情報（サンプルレート、ビット深度、OS音量）を再取得する
    fn refresh_device_info(&mut self) {
        let host = cpal::default_host();
        self.in_device_info = crate::audio::get_device_info(&host, &self.config.input_device_name, true);
        self.out1_device_info = crate::audio::get_device_info(&host, &self.config.output_device_1_name, false);
        self.out2_device_info = crate::audio::get_device_info(&host, &self.config.output_device_2_name, false);
        self.mon_device_info = crate::audio::get_device_info(&host, &self.config.monitor_device_name, false);
    }

    /// ウィンドウを非表示にし、現在位置を設定に保存する
    fn hide_and_save_position(&mut self) {
        ui_helpers::hide_window();
        self.is_hidden = true;
        if let Some((x, y)) = *self.window_pos.lock().unwrap() {
            self.config.window_pos_x = Some(x);
            self.config.window_pos_y = Some(y);
        }
        if let Some((w, h)) = *self.window_size.lock().unwrap() {
            self.config.window_size_x = Some(w);
            self.config.window_size_y = Some(h);
        }
        save_config(CONFIG_FILE, &self.config);
    }

    /// バックグラウンドで変更された AtomicBool の状態を Config に同期する
    /// 変更があった場合は設定を保存し true を返す
    fn sync_atomic_state(&mut self) -> bool {
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
            save_config(CONFIG_FILE, &self.config);
        }
        changed
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
        // デバイスの接続/切断検知
        if let Some(rx) = &self.device_refresh_rx {
            if rx.try_recv().is_ok() {
                self.input_devices = crate::audio::get_input_devices();
                self.output_devices = crate::audio::get_output_devices();
                self.rebuild_streams();
                ctx.request_repaint();
            }
        }

        // 二重起動からの表示命令を受信
        if self.should_show.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            self.is_hidden = false;
        }

        // 最小化されたら非表示にする
        if ctx.input(|i| i.viewport().minimized.unwrap_or(false)) {
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Minimized(false));
            self.hide_and_save_position();
        }

        // ウィンドウのクローズイベントをキャンセルして非表示にする
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::CancelClose);
            self.hide_and_save_position();
        }

        // ウィンドウ表示中は位置とサイズを記録し、アニメーション用に再描画を要求する
        if !self.is_hidden {
            if let Some(rect) = ctx.input(|i| i.viewport().outer_rect) {
                if rect.min.x > -10000.0 && rect.min.y > -10000.0 {
                    *self.window_pos.lock().unwrap() = Some((rect.min.x, rect.min.y));
                }
            }
            if let Some(rect) = ctx.input(|i| i.viewport().inner_rect) {
                *self.window_size.lock().unwrap() = Some((rect.width(), rect.height()));
            }
            ctx.request_repaint();
        }

        // バックグラウンドで変更された状態を Config に同期
        if self.sync_atomic_state() {
            ctx.request_repaint();
        }

        // --- メインUI描画 ---
        if !self.is_hidden {
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("MicSplitter");

                let mut config_changed = false;

                // ショートカットキー表示
                if self.draw_shortcut_settings(ui) { config_changed = true; }

                // デバイス設定
                if self.draw_device_settings(ui) { config_changed = true; }

                // エラー表示
                if let Some(err) = &self.stream_error {
                    ui.separator();
                    ui.label(eframe::egui::RichText::new(format!("エラー: {}", err)).color(eframe::egui::Color32::RED));
                }

                // ボリュームとミュート設定
                if self.draw_volume_settings(ui) { config_changed = true; }

                // 音声ルーティングフロー図
                if self.draw_routing_flow(ui) { config_changed = true; }

                // サウンド設定ボタン
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("サウンド設定を開く (コントロールパネル)").clicked() {
                        let _ = std::process::Command::new("control.exe").arg("mmsys.cpl").spawn();
                    }
                });

                if config_changed {
                    save_config(CONFIG_FILE, &self.config);
                }
            });
        }

        // バックグラウンドでもイベントを受信し続けるため、再描画をリクエスト
        ctx.request_repaint_after(std::time::Duration::from_millis(
            crate::constants::EVENT_LOOP_INTERVAL_MS,
        ));
    }
}

impl MicSplitterApp {
    fn draw_shortcut_settings(&mut self, ui: &mut eframe::egui::Ui) -> bool {
        let mut config_changed = false;
        ui.separator();
        eframe::egui::CollapsingHeader::new("ショートカットキー").default_open(true).show(ui, |ui| {
            
            // 汎用ホットキーUI描画関数
            let mut draw_hotkey = |ui: &mut eframe::egui::Ui, label: &str, target_enum: WaitingForKey, config_str: &mut String, atomic_vk: &Arc<std::sync::atomic::AtomicU32>, use_modifier: bool| {
                ui.horizontal(|ui| {
                    ui.label(label);
                    if self.waiting_for_key == target_enum {
                        let _ = ui.button("キー入力待ち... (任意のキーを押してください)");
                        
                        let mut detected_vk = None;
                        for vk in 8..=254 {
                            if matches!(vk, 1|2|4|5|6) { continue; } // マウスボタン除外
                            // Modifier自体は単独キーとして検知しない
                            if use_modifier && matches!(vk, 16|17|18|91|92|160|161|162|163|164|165) { continue; }
                            
                            unsafe {
                                let is_down = (windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(vk) as u16 & 0x8000) != 0;
                                if is_down {
                                    if let Some(initials) = self.hotkey_initial_keys {
                                        if !initials[vk as usize] {
                                            if use_modifier {
                                                let ctrl = (windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(17) as u16 & 0x8000) != 0;
                                                let shift = (windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(16) as u16 & 0x8000) != 0;
                                                let alt = (windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(18) as u16 & 0x8000) != 0;
                                                let win = ((windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(91) as u16 & 0x8000) != 0) || ((windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(92) as u16 & 0x8000) != 0);
                                                
                                                let mut packed = vk as u32;
                                                if ctrl { packed |= crate::hotkey::MOD_CTRL; }
                                                if shift { packed |= crate::hotkey::MOD_SHIFT; }
                                                if alt { packed |= crate::hotkey::MOD_ALT; }
                                                if win { packed |= crate::hotkey::MOD_WIN; }
                                                
                                                detected_vk = Some((crate::hotkey::format_hotkey(packed), packed));
                                            } else {
                                                detected_vk = Some((vk.to_string(), vk as u32));
                                            }
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        if let Some((s, packed)) = detected_vk {
                            *config_str = s;
                            atomic_vk.store(packed, Ordering::Relaxed);
                            self.waiting_for_key = WaitingForKey::None;
                            self.hotkey_initial_keys = None;
                            config_changed = true;
                        }
                    } else {
                        let key_str = if config_str.is_empty() {
                            "未設定".to_string()
                        } else if use_modifier {
                            config_str.clone()
                        } else if let Ok(vk) = config_str.parse::<u16>() {
                            crate::ptt::vk_to_string(vk)
                        } else {
                            "エラー".to_string()
                        };
                        if ui.button(format!("[ {} ] (クリックして変更)", key_str)).clicked() {
                            self.waiting_for_key = target_enum;
                            let mut initials = [false; 256];
                            for vk in 8..=254 {
                                unsafe {
                                    let is_down = (windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(vk) as u16 & 0x8000) != 0;
                                    initials[vk as usize] = is_down;
                                }
                            }
                            self.hotkey_initial_keys = Some(initials);
                        }
                    }
                });
            };

            ui.label("【グローバルショートカット (トグル・ON/OFF)】");
            draw_hotkey(ui, "入力ミュート:", WaitingForKey::InToggle, &mut self.config.input_device_hotkey, &self.in_hotkey_vk, true);
            draw_hotkey(ui, "モニターミュート:", WaitingForKey::MonToggle, &mut self.config.monitor_device_hotkey, &self.mon_hotkey_vk, true);
            draw_hotkey(ui, "出力1 (仮想マイクA) をON:", WaitingForKey::Out1Toggle, &mut self.config.output_device_1_hotkey, &self.out1_hotkey_vk, true);
            draw_hotkey(ui, "出力2 (仮想マイクB) をON:", WaitingForKey::Out2Toggle, &mut self.config.output_device_2_hotkey, &self.out2_hotkey_vk, true);

            ui.separator();
            ui.label("【プッシュ・トゥ・トーク (PTT) キー設定】");
            draw_hotkey(ui, "出力1 (仮想マイクA):", WaitingForKey::Ptt1, &mut self.config.ptt_out1_hotkey, &self.ptt_out1_vk, false);
            draw_hotkey(ui, "出力2 (仮想マイクB):", WaitingForKey::Ptt2, &mut self.config.ptt_out2_hotkey, &self.ptt_out2_vk, false);
            
        });
        config_changed
    }

    fn draw_device_settings(&mut self, ui: &mut eframe::egui::Ui) -> bool {
        let mut config_changed = false;
        ui.separator();
        eframe::egui::CollapsingHeader::new("デバイス設定").default_open(true).show(ui, |ui| {
            if ui.button("デバイスリストを更新").clicked() {
                self.input_devices = crate::audio::get_input_devices();
                self.output_devices = crate::audio::get_output_devices();
                self.refresh_device_info();
                self.rebuild_streams();
            }

            if ui_helpers::device_combo_box(ui, "in_dev", "入力デバイス:", &mut self.config.input_device_name, &self.input_devices, &self.in_device_info, None, false) {
                config_changed = true;
            }
            if ui_helpers::device_combo_box(ui, "mon_dev", "モニター出力:", &mut self.config.monitor_device_name, &self.output_devices, &self.mon_device_info, None, false) {
                config_changed = true;
            }
            if ui_helpers::device_combo_box(ui, "out1_dev", "仮想出力1:", &mut self.config.output_device_1_name, &self.output_devices, &self.out1_device_info, Some(self.config.icon_color_out1_on), true) {
                config_changed = true;
            }
            if ui_helpers::device_combo_box(ui, "out2_dev", "仮想出力2:", &mut self.config.output_device_2_name, &self.output_devices, &self.out2_device_info, Some(self.config.icon_color_out2_on), true) {
                config_changed = true;
            }

            if config_changed {
                save_config(CONFIG_FILE, &self.config);
                self.rebuild_streams();
            }
        });
        config_changed
    }

    fn draw_volume_settings(&mut self, ui: &mut eframe::egui::Ui) -> bool {
        let mut config_changed = false;
        ui.separator();
        eframe::egui::CollapsingHeader::new("ボリュームとミュート設定").default_open(true).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("切り替えモード:");
                if ui.radio_value(&mut self.config.switching_mode, "toggle".to_string(), "トグル (排他)").changed() {
                    self.is_toggle_mode.store(true, Ordering::Relaxed);
                    self.is_ptt_mode.store(false, Ordering::Relaxed);
                    // 両方OFF、または両方ONの場合は出力1のみをONにする
                    if (!self.config.output_device_1_enabled && !self.config.output_device_2_enabled)
                        || (self.config.output_device_1_enabled && self.config.output_device_2_enabled)
                    {
                        self.config.output_device_1_enabled = true;
                        self.config.output_device_2_enabled = false;
                        self.out1_enabled.store(true, Ordering::Relaxed);
                        self.out2_enabled.store(false, Ordering::Relaxed);
                    }
                    config_changed = true;
                }
                if ui.radio_value(&mut self.config.switching_mode, "individual".to_string(), "個別ON/OFF").changed() {
                    self.is_toggle_mode.store(false, Ordering::Relaxed);
                    self.is_ptt_mode.store(false, Ordering::Relaxed);
                    config_changed = true;
                }
                if ui.radio_value(&mut self.config.switching_mode, "ptt".to_string(), "プッシュトゥトーク").changed() {
                    self.is_toggle_mode.store(false, Ordering::Relaxed);
                    self.is_ptt_mode.store(true, Ordering::Relaxed);
                    // PTTモードではデフォルト両方ミュートにする
                    self.config.output_device_1_enabled = false;
                    self.config.output_device_2_enabled = false;
                    self.out1_enabled.store(false, Ordering::Relaxed);
                    self.out2_enabled.store(false, Ordering::Relaxed);
                    config_changed = true;
                }
            });

            ui.separator();

            // 入力
            ui.horizontal(|ui| {
                let is_invalid = self.in_device_info.is_none();
                let label = eframe::egui::RichText::new(format!("入力 (マイク): {}", self.config.input_device_name));
                ui.label(if is_invalid { label.color(eframe::egui::Color32::RED) } else { label });
                ui.with_layout(eframe::egui::Layout::right_to_left(eframe::egui::Align::Center), |ui| {
                    ui.horizontal(|ui| {
                        let rms = f32::from_bits(self.in_rms_bits.load(Ordering::Relaxed));
                        ui_helpers::volume_meter_ui(ui, rms, &mut self.in_meter_state, self.config.input_enabled, &self.config);
                        if ui.add_sized([130.0, 24.0], eframe::egui::Checkbox::new(&mut self.config.input_enabled, "ON (ミュート解除)")).changed() {
                            self.in_enabled.store(self.config.input_enabled, Ordering::Relaxed);
                            config_changed = true;
                        }
                    });
                });
            });

            // モニター
            ui.horizontal(|ui| {
                let is_invalid = self.mon_device_info.is_none();
                let label = eframe::egui::RichText::new(format!("モニター出力: {}", self.config.monitor_device_name));
                ui.label(if is_invalid { label.color(eframe::egui::Color32::RED) } else { label });
                ui.with_layout(eframe::egui::Layout::right_to_left(eframe::egui::Align::Center), |ui| {
                    ui.horizontal(|ui| {
                        let rms = f32::from_bits(self.mon_rms_bits.load(Ordering::Relaxed));
                        ui_helpers::volume_meter_ui(ui, rms, &mut self.mon_meter_state, self.config.monitor_enabled, &self.config);
                        if ui.add_sized([130.0, 24.0], eframe::egui::Checkbox::new(&mut self.config.monitor_enabled, "モニターON")).changed() {
                            self.mon_enabled.store(self.config.monitor_enabled, Ordering::Relaxed);
                            config_changed = true;
                        }
                    });
                });
            });
            ui.horizontal(|ui| {
                ui.with_layout(eframe::egui::Layout::right_to_left(eframe::egui::Align::Center), |ui| {
                    if ui.add(eframe::egui::Slider::new(&mut self.config.monitor_volume, 0.0..=1.0).text("モニター音量")).changed() {
                        self.mon_volume_bits.store(self.config.monitor_volume.to_bits(), Ordering::Relaxed);
                        config_changed = true;
                    }
                });
            });

            // 出力1
            ui.horizontal(|ui| {
                let is_invalid = self.config.output_device_1_name == "(未選択)" || self.out1_device_info.is_none();
                let label = eframe::egui::RichText::new(format!("出力1 (仮想マイクA): {}", self.config.output_device_1_name));
                let c = self.config.icon_color_out1_on;
                ui.label(eframe::egui::RichText::new("●").color(eframe::egui::Color32::from_rgb(c[0], c[1], c[2])));
                ui.label(if is_invalid { label.color(eframe::egui::Color32::RED) } else { label });
                ui.with_layout(eframe::egui::Layout::right_to_left(eframe::egui::Align::Center), |ui| {
                    ui.horizontal(|ui| {
                        let rms = f32::from_bits(self.out1_rms_bits.load(Ordering::Relaxed));
                        ui_helpers::volume_meter_ui(ui, rms, &mut self.out1_meter_state, self.config.output_device_1_enabled, &self.config);
                        if ui.add_sized([130.0, 24.0], eframe::egui::Checkbox::new(&mut self.config.output_device_1_enabled, "ON (ミュート解除)")).changed() {
                            if self.config.switching_mode == "toggle" && self.config.output_device_1_enabled {
                                self.config.output_device_2_enabled = false;
                                self.out2_enabled.store(false, Ordering::Relaxed);
                            }
                            self.out1_enabled.store(self.config.output_device_1_enabled, Ordering::Relaxed);
                            config_changed = true;
                        }
                    });
                });
            });

            // 出力2
            ui.horizontal(|ui| {
                let is_invalid = self.config.output_device_2_name == "(未選択)" || self.out2_device_info.is_none();
                let label = eframe::egui::RichText::new(format!("出力2 (仮想マイクB): {}", self.config.output_device_2_name));
                let c = self.config.icon_color_out2_on;
                ui.label(eframe::egui::RichText::new("●").color(eframe::egui::Color32::from_rgb(c[0], c[1], c[2])));
                ui.label(if is_invalid { label.color(eframe::egui::Color32::RED) } else { label });
                ui.with_layout(eframe::egui::Layout::right_to_left(eframe::egui::Align::Center), |ui| {
                    ui.horizontal(|ui| {
                        let rms = f32::from_bits(self.out2_rms_bits.load(Ordering::Relaxed));
                        ui_helpers::volume_meter_ui(ui, rms, &mut self.out2_meter_state, self.config.output_device_2_enabled, &self.config);
                        if ui.add_sized([130.0, 24.0], eframe::egui::Checkbox::new(&mut self.config.output_device_2_enabled, "ON (ミュート解除)")).changed() {
                            if self.config.switching_mode == "toggle" && self.config.output_device_2_enabled {
                                self.config.output_device_1_enabled = false;
                                self.out1_enabled.store(false, Ordering::Relaxed);
                            }
                            self.out2_enabled.store(self.config.output_device_2_enabled, Ordering::Relaxed);
                            config_changed = true;
                        }
                    });
                });
            });
        });
        config_changed
    }

    fn draw_routing_flow(&mut self, ui: &mut eframe::egui::Ui) -> bool {
        let mut config_changed = false;
        ui.add_space(10.0);
        ui.separator();
        eframe::egui::CollapsingHeader::new("音声ルーティングフロー図").default_open(true).show(ui, |ui| {
            let in_invalid = self.in_device_info.is_none();
            let out1_invalid = self.config.output_device_1_name == "(未選択)" || self.out1_device_info.is_none();
            let out2_invalid = self.config.output_device_2_name == "(未選択)" || self.out2_device_info.is_none();
            let mon_invalid = self.config.monitor_device_name == "(未選択)" || self.mon_device_info.is_none();

            let vmic1_name = ui_helpers::find_corresponding_virtual_mic(&self.config.output_device_1_name, &self.input_devices);
            let vmic2_name = ui_helpers::find_corresponding_virtual_mic(&self.config.output_device_2_name, &self.input_devices);
            
            let vmic1_invalid = vmic1_name.is_none();
            let vmic2_invalid = vmic2_name.is_none();

            let vmic1_display = vmic1_name.unwrap_or_else(|| "(未接続・無効)".to_string());
            let vmic2_display = vmic2_name.unwrap_or_else(|| "(未接続・無効)".to_string());

            let col1_w = 240.0;
            let col3_w = 260.0;
            let col5_w = 340.0;

            eframe::egui::Grid::new("routing_grid").num_columns(5).spacing([10.0, 10.0]).show(ui, |ui| {
                let btn_in = ui_helpers::routing_button_ui(ui, &format!("入力: {}", self.config.input_device_name), self.config.input_enabled, in_invalid, true, col1_w);
                if btn_in.clicked() {
                    self.config.input_enabled = !self.config.input_enabled;
                    self.in_enabled.store(self.config.input_enabled, Ordering::Relaxed);
                    config_changed = true;
                }
                ui.label("─▶");
                let btn_out1 = ui_helpers::routing_button_ui(ui, &format!("仮想出力1: {}", self.config.output_device_1_name), self.config.output_device_1_enabled, out1_invalid, true, col3_w);
                if btn_out1.clicked() {
                    self.config.output_device_1_enabled = !self.config.output_device_1_enabled;
                    if self.config.switching_mode == "toggle" && self.config.output_device_1_enabled {
                        self.config.output_device_2_enabled = false;
                        self.out2_enabled.store(false, Ordering::Relaxed);
                    }
                    self.out1_enabled.store(self.config.output_device_1_enabled, Ordering::Relaxed);
                    config_changed = true;
                }
                ui.label("─▶");
                ui_helpers::routing_button_ui(ui, &format!("仮想マイク1: {}", vmic1_display), self.config.output_device_1_enabled, vmic1_invalid, false, col5_w);
                ui.end_row();

                ui.label("");
                ui.label("├▶");
                let btn_out2 = ui_helpers::routing_button_ui(ui, &format!("仮想出力2: {}", self.config.output_device_2_name), self.config.output_device_2_enabled, out2_invalid, true, col3_w);
                if btn_out2.clicked() {
                    self.config.output_device_2_enabled = !self.config.output_device_2_enabled;
                    if self.config.switching_mode == "toggle" && self.config.output_device_2_enabled {
                        self.config.output_device_1_enabled = false;
                        self.out1_enabled.store(false, Ordering::Relaxed);
                    }
                    self.out2_enabled.store(self.config.output_device_2_enabled, Ordering::Relaxed);
                    config_changed = true;
                }
                ui.label("─▶");
                ui_helpers::routing_button_ui(ui, &format!("仮想マイク2: {}", vmic2_display), self.config.output_device_2_enabled, vmic2_invalid, false, col5_w);
                ui.end_row();

                ui.label("");
                ui.label("└▶");
                let btn_mon = ui_helpers::routing_button_ui(ui, &format!("モニター出力: {}", self.config.monitor_device_name), self.config.monitor_enabled, mon_invalid, true, col3_w);
                if btn_mon.clicked() {
                    self.config.monitor_enabled = !self.config.monitor_enabled;
                    self.mon_enabled.store(self.config.monitor_enabled, Ordering::Relaxed);
                    config_changed = true;
                }
                ui.label("");
                ui.label("");
                ui.end_row();
            });
        });
        config_changed
    }

}
