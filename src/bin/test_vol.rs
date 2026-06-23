use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, CoCreateInstance, COINIT_MULTITHREADED, CLSCTX_ALL, STGM_READ};
use windows::Win32::Media::Audio::{MMDeviceEnumerator, IMMDeviceEnumerator, eRender, DEVICE_STATE_ACTIVE};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::System::Variant::VT_LPWSTR;

fn main() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).unwrap();
        
        let collection = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE).unwrap();
        
        let count = collection.GetCount().unwrap();
        for i in 0..count {
            if let Ok(device) = collection.Item(i) {
                if let Ok(store) = device.OpenPropertyStore(STGM_READ) {
                    if let Ok(prop) = store.GetValue(&PKEY_Device_FriendlyName) {
                        if prop.Anonymous.Anonymous.vt == windows::Win32::System::Variant::VARENUM(VT_LPWSTR.0 as u16) {
                            let pwsz = prop.Anonymous.Anonymous.Anonymous.pwszVal;
                            if !pwsz.is_null() {
                                let name = pwsz.to_string().unwrap_or_default();
                                println!("Device: {}", name);
                                if let Ok(vol) = device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
                                    if let Ok(level) = vol.GetMasterVolumeLevelScalar() {
                                        println!("  Volume: {}", level);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        let _ = CoUninitialize();
    }
}
