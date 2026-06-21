use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

mod config;
mod app;
mod tray;
mod hotkey;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 設定ファイルの読み込み
    let cfg = config::load_config("config.json");

    let host = cpal::default_host();

    // デバイス検索
    let input_device = find_input_device(&host, &cfg.input_device_name)
        .unwrap_or_else(|| host.default_input_device().expect("入力デバイスが見つかりません"));
    let output1_device = find_output_device(&host, &cfg.output_device_1_name)
        .unwrap_or_else(|| host.default_output_device().expect("出力デバイス1が見つかりません"));
    let output2_device = find_output_device(&host, &cfg.output_device_2_name)
        .unwrap_or_else(|| host.default_output_device().expect("出力デバイス2が見つかりません"));
    let monitor_device = find_output_device(&host, &cfg.monitor_device_name)
        .unwrap_or_else(|| host.default_output_device().expect("モニターデバイスが見つかりません"));

    println!("使用デバイス:");
    println!("  Input   : {}", input_device);
    println!("  Output 1: {}", output1_device);
    println!("  Output 2: {}", output2_device);
    println!("  Monitor : {}", monitor_device);

    // デフォルトの設定を取得
    let input_config = input_device.default_input_config()?;
    let sample_rate = input_config.sample_rate();
    let channels = input_config.channels() as usize;

    println!("入力フォーマット: {} Hz, {} ch, {:?}", sample_rate, channels, input_config.sample_format());

    // バッファサイズ: 50ms 相当のフレーム数 * チャンネル数
    let latency_ms = 50.0;
    let frames = (sample_rate as f32 * (latency_ms / 1000.0)) as usize;
    let buffer_capacity = frames * channels;

    // 3つのリングバッファを作成
    let rb1 = HeapRb::<f32>::new(buffer_capacity);
    let rb2 = HeapRb::<f32>::new(buffer_capacity);
    let rb_mon = HeapRb::<f32>::new(buffer_capacity);

    let (mut prod1, cons1) = rb1.split();
    let (mut prod2, cons2) = rb2.split();
    let (mut prod_mon, cons_mon) = rb_mon.split();

    // 入力ストリームの構築
    let err_fn = |err| eprintln!("Input error: {}", err);
    let input_stream = match input_config.sample_format() {
        SampleFormat::F32 => input_device.build_input_stream(
            input_config.into(),
            move |data: &[f32], _: &_| {
                // すべてのバッファに書き込み
                for &sample in data {
                    let _ = prod1.try_push(sample);
                    let _ = prod2.try_push(sample);
                    let _ = prod_mon.try_push(sample);
                }
            },
            err_fn,
            None,
        ).map_err(|e| format!("入力デバイス ({}) のストリーム初期化に失敗: {}", input_device, e))?,
        _ => return Err("サポートされていない入力フォーマットです (f32のみ対応)".into()),
    };

    let in_enabled = Arc::new(AtomicBool::new(cfg.input_enabled));
    let out1_enabled = Arc::new(AtomicBool::new(cfg.output_device_1_enabled));
    let out2_enabled = Arc::new(AtomicBool::new(cfg.output_device_2_enabled));
    let mon_enabled = Arc::new(AtomicBool::new(cfg.monitor_enabled));

    let stream1 = build_output(&output1_device, "Output 1", cons1, channels, in_enabled.clone(), out1_enabled.clone(), 1.0)?;
    let stream2 = build_output(&output2_device, "Output 2", cons2, channels, in_enabled.clone(), out2_enabled.clone(), 1.0)?;
    let stream_mon = build_output(&monitor_device, "Monitor", cons_mon, channels, in_enabled.clone(), mon_enabled.clone(), cfg.monitor_volume)?;

    // 再生開始
    input_stream.play()?;
    stream1.play()?;
    stream2.play()?;
    stream_mon.play()?;

    println!("音声転送を開始し、GUIウィンドウを起動します。");

    let should_show = Arc::new(AtomicBool::new(false));

    let app = app::MicSplitterApp {
        config: cfg,
        in_enabled: in_enabled.clone(),
        out1_enabled: out1_enabled.clone(),
        out2_enabled: out2_enabled.clone(),
        mon_enabled: mon_enabled.clone(),
        _input_stream: input_stream,
        _stream1: stream1,
        _stream2: stream2,
        _stream_mon: stream_mon,
        is_hidden: false,
        should_show: should_show.clone(),
    };

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "MicSplitter",
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

            std::thread::spawn(move || {
                // スレッド内でトレイとホットキーを初期化
                let _tray = tray::create_tray_icon().ok();
                let hotkeys = match hotkey::register_hotkeys() {
                    Ok(h) => Some(h),
                    Err(e) => {
                        println!("ホットキーの登録に失敗しました: {:?}", e);
                        None
                    }
                };
                
                // HotkeyのIDだけを抽出してポーリングスレッドに渡す
                let (mon_id, out1_id, out2_id, in_id, swap_id) = if let Some(ref h) = hotkeys {
                    (Some(h.toggle_mon_id), Some(h.toggle_out1_id), Some(h.toggle_out2_id), Some(h.toggle_in_id), Some(h.toggle_swap_id))
                } else {
                    (None, None, None, None, None)
                };

                // イベントポーリング用スレッドをさらに分ける
                // (GetMessageW はブロックするため、try_recv 監視用にもう一つスレッドを立てるのが簡単)
                std::thread::spawn(move || {
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        
                        // トレイクリック (キューに溜まったイベントをすべて消化する)
                        while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
                            if let tray_icon::TrayIconEvent::Click { button: tray_icon::MouseButton::Left, .. } = event {
                                unsafe {
                                    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, ShowWindow, SW_RESTORE, SW_SHOW, SetForegroundWindow};
                                    let window_name: Vec<u16> = "MicSplitter\0".encode_utf16().collect();
                                    let hwnd = FindWindowW(std::ptr::null(), window_name.as_ptr());
                                    if hwnd != std::ptr::null_mut() {
                                        ShowWindow(hwnd, SW_RESTORE);
                                        ShowWindow(hwnd, SW_SHOW);
                                        SetForegroundWindow(hwnd);
                                    }
                                }
                                should_show_clone.store(true, Ordering::SeqCst);
                                ctx_clone.send_viewport_cmd(eframe::egui::ViewportCommand::Visible(true));
                                ctx_clone.request_repaint();
                            }
                        }

                        // トレイメニュー (キューに溜まったイベントをすべて消化する)
                        while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
                            if event.id.0 == "show" {
                                unsafe {
                                    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, ShowWindow, SW_RESTORE, SW_SHOW, SetForegroundWindow};
                                    let window_name: Vec<u16> = "MicSplitter\0".encode_utf16().collect();
                                    let hwnd = FindWindowW(std::ptr::null(), window_name.as_ptr());
                                    if hwnd != std::ptr::null_mut() {
                                        ShowWindow(hwnd, SW_RESTORE);
                                        ShowWindow(hwnd, SW_SHOW);
                                        SetForegroundWindow(hwnd);
                                    }
                                }
                                should_show_clone.store(true, Ordering::SeqCst);
                                ctx_clone.send_viewport_cmd(eframe::egui::ViewportCommand::Visible(true));
                                ctx_clone.request_repaint();
                            } else if event.id.0 == "quit" {
                                std::process::exit(0);
                            }
                        }

                        // グローバルホットキー (キューに溜まったイベントをすべて消化する)
                        while let Ok(event) = global_hotkey::GlobalHotKeyEvent::receiver().try_recv() {
                            if event.state == global_hotkey::HotKeyState::Pressed {
                                if Some(event.id) == mon_id {
                                    mon_clone.store(!mon_clone.load(Ordering::Relaxed), Ordering::Relaxed);
                                } else if Some(event.id) == out1_id {
                                    out1_clone.store(!out1_clone.load(Ordering::Relaxed), Ordering::Relaxed);
                                } else if Some(event.id) == out2_id {
                                    out2_clone.store(!out2_clone.load(Ordering::Relaxed), Ordering::Relaxed);
                                } else if Some(event.id) == in_id {
                                    in_clone.store(!in_clone.load(Ordering::Relaxed), Ordering::Relaxed);
                                } else if Some(event.id) == swap_id {
                                    let out1 = out1_clone.load(Ordering::Relaxed);
                                    let out2 = out2_clone.load(Ordering::Relaxed);
                                    if out1 && !out2 {
                                        out1_clone.store(false, Ordering::Relaxed);
                                        out2_clone.store(true, Ordering::Relaxed);
                                    } else {
                                        out1_clone.store(true, Ordering::Relaxed);
                                        out2_clone.store(false, Ordering::Relaxed);
                                    }
                                }
                                ctx_clone.request_repaint();
                            }
                        }
                    }
                });

                // OSメッセージループを回す
                use windows_sys::Win32::UI::WindowsAndMessaging::{GetMessageW, DispatchMessageW, TranslateMessage, MSG};
                unsafe {
                    let mut msg: MSG = std::mem::zeroed();
                    while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            });

            Ok(Box::new(app))
        }),
    ).map_err(|e| e.into())
}

fn build_output<C>(
    device: &cpal::Device,
    name: &str,
    mut consumer: C,
    in_channels: usize,
    in_enabled: Arc<AtomicBool>,
    enabled: Arc<AtomicBool>,
    volume: f32,
) -> Result<cpal::Stream, Box<dyn std::error::Error>>
where
    C: Consumer<Item = f32> + Send + 'static,
{
    let out_config = device.default_output_config()
        .map_err(|e| format!("デバイス ({}) のデフォルト出力設定取得に失敗: {}", name, e))?;
    let out_channels = out_config.channels() as usize;

    println!("出力フォーマット ({}): {} Hz, {} ch, {:?}", name, out_config.sample_rate(), out_channels, out_config.sample_format());

    let name_str = name.to_string();
    let err_fn = move |err| eprintln!("Output error ({}): {}", name_str, err);

    let name_for_err = name.to_string();
    let device_name = device.to_string();

    match out_config.sample_format() {
        SampleFormat::F32 => {
            let stream = device.build_output_stream(
                out_config.into(),
                move |data: &mut [f32], _: &_| {
                    let is_in_enabled = in_enabled.load(Ordering::Relaxed);
                    let is_enabled = enabled.load(Ordering::Relaxed);
                    let is_active = is_in_enabled && is_enabled;
                    let mut input_buffer = vec![0.0; in_channels];
                    for frame in data.chunks_mut(out_channels) {
                        for i in 0..in_channels {
                            if let Some(s) = consumer.try_pop() {
                                input_buffer[i] = if is_active { s * volume } else { 0.0 };
                            } else {
                                input_buffer[i] = 0.0;
                            }
                        }

                        for out_c in 0..out_channels {
                            let in_c = if out_c < in_channels { out_c } else { 0 };
                            frame[out_c] = input_buffer[in_c];
                        }
                    }
                },
                err_fn,
                None,
            ).map_err(|e| format!("出力デバイス ({}: {}) のストリーム初期化に失敗: {}", name_for_err, device_name, e))?;
            Ok(stream)
        }
        _ => Err(format!("出力デバイス ({}) でサポートされていないフォーマット (f32のみ対応)", name).into()),
    }
}

fn find_input_device(host: &cpal::Host, keyword: &str) -> Option<cpal::Device> {
    if let Ok(devices) = host.input_devices() {
        for device in devices {
            let name = device.to_string();
            if name.contains(keyword) {
                return Some(device);
            }
        }
    }
    None
}

fn find_output_device(host: &cpal::Host, keyword: &str) -> Option<cpal::Device> {
    if let Ok(devices) = host.output_devices() {
        for device in devices {
            let name = device.to_string();
            if name.contains(keyword) {
                return Some(device);
            }
        }
    }
    None
}
