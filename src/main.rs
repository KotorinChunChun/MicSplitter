use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

mod config;
mod constants;
mod app;
mod tray;
mod hotkey;
mod audio;
mod ui_helpers;
mod ptt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. コマンドライン引数の解析
    let args: Vec<String> = std::env::args().collect();
    let is_autostart = args.iter().any(|a| a == "--autostart");

    // 2. 単一インスタンス制限 (Named Mutex)
    unsafe {
        use windows_sys::Win32::System::Threading::CreateMutexW;
        use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
        let mutex_name: Vec<u16> = format!("{}\0", constants::MUTEX_NAME).encode_utf16().collect();
        let h_mutex = CreateMutexW(std::ptr::null(), 0, mutex_name.as_ptr());
        
        let is_first = if h_mutex == std::ptr::null_mut() {
            true
        } else {
            GetLastError() != ERROR_ALREADY_EXISTS
        };

        if !is_first {
            // 二重起動時は、既存プロセスへ表示命令を投げて終了
            if let Ok(socket) = std::net::UdpSocket::bind("127.0.0.1:0") {
                let _ = socket.send_to(b"show", constants::IPC_ADDR);
            }
            return Ok(());
        }
    }

    // 3. UDPリスナーの設定 (既存プロセス用)
    let udp_receiver = std::net::UdpSocket::bind(constants::IPC_ADDR).ok();
    if let Some(ref socket) = udp_receiver {
        let _ = socket.set_nonblocking(true);
    }

    let cfg = config::load_config(constants::CONFIG_FILE);

    let in_enabled = Arc::new(AtomicBool::new(cfg.input_enabled));
    let out1_enabled = Arc::new(AtomicBool::new(cfg.output_device_1_enabled));
    let out2_enabled = Arc::new(AtomicBool::new(cfg.output_device_2_enabled));
    let mon_enabled = Arc::new(AtomicBool::new(cfg.monitor_enabled));

    let is_toggle_mode = Arc::new(AtomicBool::new(cfg.switching_mode == "toggle"));
    let is_ptt_mode = Arc::new(AtomicBool::new(cfg.switching_mode == "ptt"));
    let ptt_out1_vk = Arc::new(std::sync::atomic::AtomicU32::new(cfg.ptt_out1_hotkey.parse::<u32>().unwrap_or(0)));
    let ptt_out2_vk = Arc::new(std::sync::atomic::AtomicU32::new(cfg.ptt_out2_hotkey.parse::<u32>().unwrap_or(0)));
    let in_hotkey_vk = Arc::new(std::sync::atomic::AtomicU32::new(hotkey::parse_hotkey(&cfg.input_device_hotkey)));
    let mon_hotkey_vk = Arc::new(std::sync::atomic::AtomicU32::new(hotkey::parse_hotkey(&cfg.monitor_device_hotkey)));
    let out1_hotkey_vk = Arc::new(std::sync::atomic::AtomicU32::new(hotkey::parse_hotkey(&cfg.output_device_1_hotkey)));
    let out2_hotkey_vk = Arc::new(std::sync::atomic::AtomicU32::new(hotkey::parse_hotkey(&cfg.output_device_2_hotkey)));
    let mon_volume_bits = Arc::new(std::sync::atomic::AtomicU32::new(cfg.monitor_volume.to_bits()));

    let in_rms_bits = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let out1_rms_bits = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let out2_rms_bits = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let mon_rms_bits = Arc::new(std::sync::atomic::AtomicU32::new(0));

    let should_show = Arc::new(AtomicBool::new(false));
    let window_pos = Arc::new(std::sync::Mutex::new(
        if let (Some(x), Some(y)) = (cfg.window_pos_x, cfg.window_pos_y) { Some((x, y)) } else { None }
    ));
    let window_size = Arc::new(std::sync::Mutex::new(
        if let (Some(w), Some(h)) = (cfg.window_size_x, cfg.window_size_y) { Some((w, h)) } else { None }
    ));

    let streams = audio::build_all_streams(
        &cfg,
        in_enabled.clone(),
        out1_enabled.clone(),
        out2_enabled.clone(),
        mon_enabled.clone(),
        mon_volume_bits.clone(),
        in_rms_bits.clone(),
        out1_rms_bits.clone(),
        out2_rms_bits.clone(),
        mon_rms_bits.clone(),
    )?;

    let host = cpal::default_host();
    let in_device_info = audio::get_device_info(&host, &cfg.input_device_name, true);
    let out1_device_info = audio::get_device_info(&host, &cfg.output_device_1_name, false);
    let out2_device_info = audio::get_device_info(&host, &cfg.output_device_2_name, false);
    let mon_device_info = audio::get_device_info(&host, &cfg.monitor_device_name, false);

    let (device_refresh_tx, device_refresh_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut last_inputs = audio::get_input_devices();
        let mut last_outputs = audio::get_output_devices();
        loop {
            std::thread::sleep(std::time::Duration::from_secs(constants::DEVICE_POLL_INTERVAL_SECS));
            let inputs = audio::get_input_devices();
            let outputs = audio::get_output_devices();
            if inputs != last_inputs || outputs != last_outputs {
                last_inputs = inputs;
                last_outputs = outputs;
                let _ = device_refresh_tx.send(());
            }
        }
    });

    let app = app::MicSplitterApp {
        config: cfg.clone(),
        in_enabled: in_enabled.clone(),
        out1_enabled: out1_enabled.clone(),
        out2_enabled: out2_enabled.clone(),
        mon_enabled: mon_enabled.clone(),
        is_toggle_mode: is_toggle_mode.clone(),
        mon_volume_bits: mon_volume_bits.clone(),
        in_rms_bits,
        out1_rms_bits,
        out2_rms_bits,
        mon_rms_bits,
        _input_stream: Some(streams.input_stream),
        _stream1: streams.stream1,
        _stream2: streams.stream2,
        _stream_mon: streams.stream_mon,
        stream_error: None,
        input_devices: audio::get_input_devices(),
        output_devices: audio::get_output_devices(),
        is_hidden: is_autostart,
        should_show: should_show.clone(),
        in_meter_state: ui_helpers::VolumeMeterState::default(),
        out1_meter_state: ui_helpers::VolumeMeterState::default(),
        out2_meter_state: ui_helpers::VolumeMeterState::default(),
        mon_meter_state: ui_helpers::VolumeMeterState::default(),
        in_device_info,
        out1_device_info,
        out2_device_info,
        mon_device_info,
        device_refresh_rx: Some(device_refresh_rx),
        window_pos: window_pos.clone(),
        window_size: window_size.clone(),
        waiting_for_key: app::WaitingForKey::None,
        hotkey_initial_keys: None,
        is_ptt_mode: is_ptt_mode.clone(),
        ptt_out1_vk: ptt_out1_vk.clone(),
        ptt_out2_vk: ptt_out2_vk.clone(),
        in_hotkey_vk: in_hotkey_vk.clone(),
        mon_hotkey_vk: mon_hotkey_vk.clone(),
        out1_hotkey_vk: out1_hotkey_vk.clone(),
        out2_hotkey_vk: out2_hotkey_vk.clone(),
    };

    let mut native_options = eframe::NativeOptions::default();
    let mut viewport = eframe::egui::ViewportBuilder::default().with_visible(!is_autostart);

    if let (Some(mut x), Some(mut y)) = (cfg.window_pos_x, cfg.window_pos_y) {
        unsafe {
            use windows_sys::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONULL};
            use windows_sys::Win32::Foundation::POINT;
            let pt = POINT { x: x as i32, y: y as i32 };
            let hmon = MonitorFromPoint(pt, MONITOR_DEFAULTTONULL);
            if hmon == std::ptr::null_mut() {
                x = 0.0;
                y = 0.0;
            }
        }
        viewport = viewport.with_position(eframe::egui::pos2(x, y));
    }
    if let (Some(w), Some(h)) = (cfg.window_size_x, cfg.window_size_y) {
        viewport = viewport.with_inner_size(eframe::egui::vec2(w, h));
    }
    native_options.viewport = viewport;

    eframe::run_native(
        constants::APP_NAME,
        native_options,
        Box::new(|cc| {
            // 日本語フォントをロード
            app::setup_custom_fonts(&cc.egui_ctx);

            // イベント監視とOSメッセージループのための専用スレッド
            let ctx_clone = cc.egui_ctx.clone();
            
            // 状態変更用のArcクローン
            let in_clone = in_enabled.clone();
            let out1_clone = out1_enabled.clone();
            let out2_clone = out2_enabled.clone();
            let mon_clone = mon_enabled.clone();
            let should_show_clone = should_show.clone();
            let window_pos_clone = window_pos.clone();
            let window_size_clone = window_size.clone();
            let is_toggle_mode_clone = is_toggle_mode.clone();
            let is_ptt_mode_clone = is_ptt_mode.clone();
            let ptt_out1_vk_clone = ptt_out1_vk.clone();
            let ptt_out2_vk_clone = ptt_out2_vk.clone();
            let in_hotkey_vk_clone = in_hotkey_vk.clone();
            let mon_hotkey_vk_clone = mon_hotkey_vk.clone();
            let out1_hotkey_vk_clone = out1_hotkey_vk.clone();
            let out2_hotkey_vk_clone = out2_hotkey_vk.clone();
            let udp_receiver = udp_receiver;
            let cfg_clone = cfg.clone();

            std::thread::spawn(move || {
                // スレッド内でトレイとホットキーを初期化
                let initial_in = in_clone.load(Ordering::Relaxed);
                let initial_out1 = out1_clone.load(Ordering::Relaxed);
                let initial_out2 = out2_clone.load(Ordering::Relaxed);
                let tray_opt = tray::create_tray_icon(initial_in, initial_out1, initial_out2, &cfg_clone).ok();

                use windows_sys::Win32::UI::WindowsAndMessaging::{PeekMessageW, DispatchMessageW, TranslateMessage, MSG, PM_REMOVE};
                unsafe {
                    let mut msg: MSG = std::mem::zeroed();
                    let mut last_in = initial_in;
                    let mut last_out1 = initial_out1;
                    let mut last_out2 = initial_out2;
                    let mut was_ptt1_pressed = false;
                    let mut was_ptt2_pressed = false;
                    let mut was_in_hotkey_pressed = false;
                    let mut was_mon_hotkey_pressed = false;
                    let mut was_out1_hotkey_pressed = false;
                    let mut was_out2_hotkey_pressed = false;

                    loop {
                        while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) > 0 {
                            if msg.message == 0x0012 /* WM_QUIT */ {
                                return;
                            }
                            TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }

                        // トレイアイコンの状態を同期
                        let current_in = in_clone.load(Ordering::Relaxed);
                        let current_out1 = out1_clone.load(Ordering::Relaxed);
                        let current_out2 = out2_clone.load(Ordering::Relaxed);
                        if current_in != last_in || current_out1 != last_out1 || current_out2 != last_out2 {
                            if let Some(ref tray) = tray_opt {
                                tray::update_tray_icon(&tray.tray_icon, current_in, current_out1, current_out2, &cfg_clone);
                            }
                            last_in = current_in;
                            last_out1 = current_out1;
                            last_out2 = current_out2;
                        }

                        // UDPによる二重起動表示コマンドの受信
                        if let Some(ref socket) = udp_receiver {
                            let mut buf = [0; 16];
                            if let Ok((size, _)) = socket.recv_from(&mut buf) {
                                if &buf[..size] == b"show" {
                                    should_show_clone.store(true, Ordering::SeqCst);
                                    ctx_clone.send_viewport_cmd(eframe::egui::ViewportCommand::Visible(true));
                                    ctx_clone.request_repaint();
                                }
                            }
                        }

                        // トレイクリック（キューに溜まったイベントをすべて消化する）
                        while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
                            if let tray_icon::TrayIconEvent::Click { button: tray_icon::MouseButton::Left, .. } = event {
                                ui_helpers::show_and_focus_window();
                                should_show_clone.store(true, Ordering::SeqCst);
                                ctx_clone.send_viewport_cmd(eframe::egui::ViewportCommand::Visible(true));
                                ctx_clone.request_repaint();
                            }
                        }

                        // トレイメニュー（キューに溜まったイベントをすべて消化する）
                        while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
                            if event.id.0 == "show" {
                                ui_helpers::show_and_focus_window();
                                should_show_clone.store(true, Ordering::SeqCst);
                                ctx_clone.send_viewport_cmd(eframe::egui::ViewportCommand::Visible(true));
                                ctx_clone.request_repaint();
                            } else if event.id.0 == "quit" {
                                let mut cfg_to_save = config::load_config(constants::CONFIG_FILE);
                                if let Some((x, y)) = *window_pos_clone.lock().unwrap() {
                                    cfg_to_save.window_pos_x = Some(x);
                                    cfg_to_save.window_pos_y = Some(y);
                                }
                                if let Some((w, h)) = *window_size_clone.lock().unwrap() {
                                    cfg_to_save.window_size_x = Some(w);
                                    cfg_to_save.window_size_y = Some(h);
                                }
                                config::save_config(constants::CONFIG_FILE, &cfg_to_save);
                                std::process::exit(0);
                            }
                        }

                        // グローバルホットキー（カスタム検知）
                        let hk_in = in_hotkey_vk_clone.load(Ordering::Relaxed);
                        if hk_in > 0 {
                            let is_pressed = hotkey::is_hotkey_pressed(hk_in);
                            if is_pressed && !was_in_hotkey_pressed {
                                in_clone.store(!in_clone.load(Ordering::Relaxed), Ordering::Relaxed);
                                ctx_clone.request_repaint();
                            }
                            was_in_hotkey_pressed = is_pressed;
                        }

                        let hk_mon = mon_hotkey_vk_clone.load(Ordering::Relaxed);
                        if hk_mon > 0 {
                            let is_pressed = hotkey::is_hotkey_pressed(hk_mon);
                            if is_pressed && !was_mon_hotkey_pressed {
                                mon_clone.store(!mon_clone.load(Ordering::Relaxed), Ordering::Relaxed);
                                ctx_clone.request_repaint();
                            }
                            was_mon_hotkey_pressed = is_pressed;
                        }

                        let hk_out1 = out1_hotkey_vk_clone.load(Ordering::Relaxed);
                        if hk_out1 > 0 {
                            let is_pressed = hotkey::is_hotkey_pressed(hk_out1);
                            if is_pressed && !was_out1_hotkey_pressed {
                                if is_toggle_mode_clone.load(Ordering::Relaxed) {
                                    out1_clone.store(true, Ordering::Relaxed);
                                    out2_clone.store(false, Ordering::Relaxed);
                                } else {
                                    out1_clone.store(!out1_clone.load(Ordering::Relaxed), Ordering::Relaxed);
                                }
                                ctx_clone.request_repaint();
                            }
                            was_out1_hotkey_pressed = is_pressed;
                        }

                        let hk_out2 = out2_hotkey_vk_clone.load(Ordering::Relaxed);
                        if hk_out2 > 0 {
                            let is_pressed = hotkey::is_hotkey_pressed(hk_out2);
                            if is_pressed && !was_out2_hotkey_pressed {
                                if is_toggle_mode_clone.load(Ordering::Relaxed) {
                                    out1_clone.store(false, Ordering::Relaxed);
                                    out2_clone.store(true, Ordering::Relaxed);
                                } else {
                                    out2_clone.store(!out2_clone.load(Ordering::Relaxed), Ordering::Relaxed);
                                }
                                ctx_clone.request_repaint();
                            }
                            was_out2_hotkey_pressed = is_pressed;
                        }

                        // PTT (Push To Talk) の監視
                        use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
                        
                        if is_ptt_mode_clone.load(Ordering::Relaxed) {
                            let ptt1_vk = ptt_out1_vk_clone.load(Ordering::Relaxed);
                            if ptt1_vk > 0 {
                                let is_pressed = (GetAsyncKeyState(ptt1_vk as i32) as u16 & 0x8000) != 0;
                                if is_pressed && !was_ptt1_pressed {
                                    out1_clone.store(true, Ordering::Relaxed);
                                    ctx_clone.request_repaint();
                                } else if !is_pressed && was_ptt1_pressed {
                                    out1_clone.store(false, Ordering::Relaxed);
                                    ctx_clone.request_repaint();
                                }
                                was_ptt1_pressed = is_pressed;
                            }

                            let ptt2_vk = ptt_out2_vk_clone.load(Ordering::Relaxed);
                            if ptt2_vk > 0 {
                                let is_pressed = (GetAsyncKeyState(ptt2_vk as i32) as u16 & 0x8000) != 0;
                                if is_pressed && !was_ptt2_pressed {
                                    out2_clone.store(true, Ordering::Relaxed);
                                    ctx_clone.request_repaint();
                                } else if !is_pressed && was_ptt2_pressed {
                                    out2_clone.store(false, Ordering::Relaxed);
                                    ctx_clone.request_repaint();
                                }
                                was_ptt2_pressed = is_pressed;
                            }
                        } else {
                            // PTTモードではない場合は状態をリセット
                            was_ptt1_pressed = false;
                            was_ptt2_pressed = false;
                        }

                        std::thread::sleep(std::time::Duration::from_millis(constants::EVENT_LOOP_INTERVAL_MS));
                    }
                }
            });

            Ok(Box::new(app))
        }),
    ).map_err(|e| e.into())
}
