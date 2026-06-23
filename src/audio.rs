use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use std::sync::{Arc, atomic::{AtomicBool, Ordering, AtomicU32}};

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, CoCreateInstance, COINIT_MULTITHREADED, CLSCTX_ALL, STGM_READ};
use windows::Win32::Media::Audio::{MMDeviceEnumerator, IMMDeviceEnumerator, eRender, eCapture, DEVICE_STATE_ACTIVE};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::System::Variant::{VT_LPWSTR, VARENUM};

use crate::config::Config;

pub struct AudioStreams {
    pub input_stream: cpal::Stream,
    pub stream1: Option<cpal::Stream>,
    pub stream2: Option<cpal::Stream>,
    pub stream_mon: Option<cpal::Stream>,
}

fn get_device_sort_key(name: &str) -> (String, String) {
    if let Some(start) = name.find("(") {
        if let Some(end) = name.rfind(")") {
            if end > start {
                let hardware = name[start + 1..end].trim().to_string();
                let endpoint = name[..start].trim().to_string();
                return (hardware, endpoint);
            }
        }
    }
    ("".to_string(), name.to_string())
}

pub fn get_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    let mut list = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for device in devices {
            let name = device.to_string();
            list.push(name);
        }
    }
    list.sort_by(|a, b| {
        let key_a = get_device_sort_key(a);
        let key_b = get_device_sort_key(b);
        key_a.cmp(&key_b)
    });
    list
}

pub fn get_output_devices() -> Vec<String> {
    let host = cpal::default_host();
    let mut list = Vec::new();
    if let Ok(devices) = host.output_devices() {
        for device in devices {
            let name = device.to_string();
            list.push(name);
        }
    }
    list.sort_by(|a, b| {
        let key_a = get_device_sort_key(a);
        let key_b = get_device_sort_key(b);
        key_a.cmp(&key_b)
    });
    list
}

pub fn find_device(host: &cpal::Host, keyword: &str, is_input: bool) -> Option<cpal::Device> {
    let devices = if is_input { host.input_devices().ok()? } else { host.output_devices().ok()? };
    for device in devices {
        let name = device.to_string();
        if name.contains(keyword) {
            return Some(device);
        }
    }
    None
}

pub fn get_os_volume_internal(device_name: &str, is_input: bool) -> Option<f32> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        
        let data_flow = if is_input { eCapture } else { eRender };
        let collection = enumerator.EnumAudioEndpoints(data_flow, DEVICE_STATE_ACTIVE).ok()?;
        
        let count = collection.GetCount().ok()?;
        for i in 0..count {
            if let Ok(device) = collection.Item(i) {
                if let Ok(store) = device.OpenPropertyStore(STGM_READ) {
                    if let Ok(prop) = store.GetValue(&PKEY_Device_FriendlyName) {
                        if prop.Anonymous.Anonymous.vt == VARENUM(VT_LPWSTR.0 as u16) {
                            let pwsz = prop.Anonymous.Anonymous.Anonymous.pwszVal;
                            if !pwsz.is_null() {
                                let name = pwsz.to_string().unwrap_or_default();
                                if name == device_name || name.starts_with(device_name) {
                                    if let Ok(vol) = device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
                                        if let Ok(level) = vol.GetMasterVolumeLevelScalar() {
                                            let _ = CoUninitialize();
                                            return Some(level);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        let _ = CoUninitialize();
        None
    }
}
pub fn get_device_info(host: &cpal::Host, name: &str, is_input: bool) -> Option<(u32, String, f32)> {
    let device = find_device(host, name, is_input)?;
    let config = if is_input { device.default_input_config().ok()? } else { device.default_output_config().ok()? };
    
    let sample_rate = config.sample_rate();

    let bit_depth = match config.sample_format() {
        cpal::SampleFormat::I8 | cpal::SampleFormat::U8 => "8-bit",
        cpal::SampleFormat::I16 | cpal::SampleFormat::U16 => "16-bit",
        cpal::SampleFormat::I32 | cpal::SampleFormat::U32 => "32-bit",
        cpal::SampleFormat::I64 | cpal::SampleFormat::U64 => "64-bit",
        cpal::SampleFormat::F32 => "32-bit (Float)",
        cpal::SampleFormat::F64 => "64-bit (Float)",
        _ => "Unknown",
    }.to_string();

    let os_volume = get_os_volume_internal(name, is_input).unwrap_or(1.0);
    Some((sample_rate, bit_depth, os_volume))
}

pub fn build_all_streams(
    cfg: &Config,
    in_enabled: Arc<AtomicBool>,
    out1_enabled: Arc<AtomicBool>,
    out2_enabled: Arc<AtomicBool>,
    mon_enabled: Arc<AtomicBool>,
    mon_volume_bits: Arc<AtomicU32>,
    in_rms: Arc<AtomicU32>,
    out1_rms: Arc<AtomicU32>,
    out2_rms: Arc<AtomicU32>,
    mon_rms: Arc<AtomicU32>,
) -> Result<AudioStreams, String> {
    let host = cpal::default_host();

    let input_device = find_device(&host, &cfg.input_device_name, true)
        .ok_or("入力デバイスが見つかりません")?;
    let output1_device = find_device(&host, &cfg.output_device_1_name, false);
    let output2_device = find_device(&host, &cfg.output_device_2_name, false);
    let monitor_device = find_device(&host, &cfg.monitor_device_name, false);

    let input_config = input_device.default_input_config()
        .map_err(|e| format!("入力デバイスのデフォルト設定取得に失敗: {}", e))?;
    let sample_rate = input_config.sample_rate();
    let channels = input_config.channels() as usize;

    let latency_ms = crate::constants::AUDIO_LATENCY_MS;
    let frames = (sample_rate as f32 * (latency_ms / 1000.0)) as usize;
    let buffer_capacity = frames * channels;

    let rb1 = HeapRb::<f32>::new(buffer_capacity);
    let rb2 = HeapRb::<f32>::new(buffer_capacity);
    let rb_mon = HeapRb::<f32>::new(buffer_capacity);

    let (mut prod1, cons1) = rb1.split();
    let (mut prod2, cons2) = rb2.split();
    let (mut prod_mon, cons_mon) = rb_mon.split();

    let err_fn = |err| eprintln!("Input error: {}", err);
    let in_name = input_device.to_string();
    
    let input_stream = match input_config.sample_format() {
        SampleFormat::F32 => input_device.build_input_stream(
            input_config.into(),
            move |data: &[f32], _: &_| {
                let mut sum_sq = 0.0;
                for &sample in data {
                    sum_sq += sample * sample;
                    let _ = prod1.try_push(sample);
                    let _ = prod2.try_push(sample);
                    let _ = prod_mon.try_push(sample);
                }
                let rms = if data.is_empty() { 0.0 } else { (sum_sq / data.len() as f32).sqrt() };
                in_rms.store(rms.to_bits(), Ordering::Relaxed);
            },
            err_fn,
            None,
        ).map_err(|e| format!("入力デバイス ({}) のストリーム初期化に失敗: {}", in_name, e))?,
        _ => return Err("サポートされていない入力フォーマットです (f32のみ対応)".into()),
    };

    let stream1 = output1_device.and_then(|d| build_output(&d, "Output 1", cons1, channels, in_enabled.clone(), out1_enabled.clone(), None, out1_rms).ok());
    let stream2 = output2_device.and_then(|d| build_output(&d, "Output 2", cons2, channels, in_enabled.clone(), out2_enabled.clone(), None, out2_rms).ok());
    let stream_mon = monitor_device.and_then(|d| build_output(&d, "Monitor", cons_mon, channels, in_enabled.clone(), mon_enabled.clone(), Some(mon_volume_bits), mon_rms).ok());

    input_stream.play().map_err(|e| e.to_string())?;
    if let Some(s) = &stream1 { let _ = s.play(); }
    if let Some(s) = &stream2 { let _ = s.play(); }
    if let Some(s) = &stream_mon { let _ = s.play(); }

    Ok(AudioStreams {
        input_stream,
        stream1,
        stream2,
        stream_mon,
    })
}

fn build_output<C>(
    device: &cpal::Device,
    name: &str,
    mut consumer: C,
    in_channels: usize,
    in_enabled: Arc<AtomicBool>,
    enabled: Arc<AtomicBool>,
    volume: Option<Arc<std::sync::atomic::AtomicU32>>,
    rms_out: Arc<std::sync::atomic::AtomicU32>,
) -> Result<cpal::Stream, String>
where
    C: Consumer<Item = f32> + Send + 'static,
{
    let out_config = device.default_output_config()
        .map_err(|e| format!("デバイス ({}) のデフォルト出力設定取得に失敗: {}", name, e))?;
    let out_channels = out_config.channels() as usize;

    let name_str = name.to_string();
    let err_fn = move |err| eprintln!("Output error ({}): {}", name_str, err);
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
                    let mut sum_sq = 0.0;
                    for frame in data.chunks_mut(out_channels) {
                        for i in 0..in_channels {
                            if let Some(s) = consumer.try_pop() {
                                let v = if let Some(ref vol_arc) = volume {
                                    f32::from_bits(vol_arc.load(Ordering::Relaxed))
                                } else {
                                    1.0
                                };
                                input_buffer[i] = if is_active { s * v } else { 0.0 };
                            } else {
                                input_buffer[i] = 0.0;
                            }
                        }

                        for out_c in 0..out_channels {
                            let in_c = if out_c < in_channels { out_c } else { 0 };
                            let val = input_buffer[in_c];
                            frame[out_c] = val;
                            sum_sq += val * val;
                        }
                    }
                    let rms = if data.is_empty() { 0.0 } else { (sum_sq / data.len() as f32).sqrt() };
                    rms_out.store(rms.to_bits(), Ordering::Relaxed);
                },
                err_fn,
                None,
            ).map_err(|e| format!("出力デバイス ({}: {}) のストリーム初期化に失敗: {}", name, device_name, e))?;
            Ok(stream)
        }
        _ => Err(format!("出力デバイス ({}) でサポートされていないフォーマット (f32のみ対応)", name)),
    }
}
