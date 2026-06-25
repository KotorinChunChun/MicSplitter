use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN,
};

pub const MOD_CTRL: u32 = 1 << 16;
pub const MOD_SHIFT: u32 = 1 << 17;
pub const MOD_ALT: u32 = 1 << 18;
pub const MOD_WIN: u32 = 1 << 19;

/// "Ctrl+Alt+Win+F9" のような文字列を内部の u32 に変換する
pub fn parse_hotkey(s: &str) -> u32 {
    let mut packed = 0;
    let parts: Vec<&str> = s.split('+').map(|s| s.trim()).collect();
    if parts.is_empty() { return 0; }
    
    let key_str = parts.last().unwrap();
    if key_str.is_empty() { return 0; }
    
    let vk = crate::ptt::string_to_vk(key_str);
    if vk == 0 { return 0; }
    
    packed |= vk as u32;

    for &part in parts.iter().take(parts.len().saturating_sub(1)) {
        match part.to_uppercase().as_str() {
            "CTRL" => packed |= MOD_CTRL,
            "SHIFT" => packed |= MOD_SHIFT,
            "ALT" => packed |= MOD_ALT,
            "WIN" | "SUPER" => packed |= MOD_WIN,
            _ => {}
        }
    }
    
    packed
}

/// 内部の u32 を "Ctrl+Alt+Win+F9" のような文字列に変換する
pub fn format_hotkey(packed: u32) -> String {
    let vk = (packed & 0xFFFF) as u16;
    if vk == 0 { return String::new(); }
    
    let mut parts = Vec::new();
    if packed & MOD_CTRL != 0 { parts.push("Ctrl".to_string()); }
    if packed & MOD_SHIFT != 0 { parts.push("Shift".to_string()); }
    if packed & MOD_ALT != 0 { parts.push("Alt".to_string()); }
    if packed & MOD_WIN != 0 { parts.push("Win".to_string()); }
    
    parts.push(crate::ptt::vk_to_string(vk));
    
    parts.join("+")
}

/// u32 で表現されたホットキーが現在押されているか判定する
pub fn is_hotkey_pressed(packed: u32) -> bool {
    let vk = (packed & 0xFFFF) as i32;
    if vk == 0 { return false; }

    unsafe {
        let is_key_down = (GetAsyncKeyState(vk) as u16 & 0x8000) != 0;
        if !is_key_down { return false; }

        let req_ctrl = (packed & MOD_CTRL) != 0;
        let req_shift = (packed & MOD_SHIFT) != 0;
        let req_alt = (packed & MOD_ALT) != 0;
        let req_win = (packed & MOD_WIN) != 0;

        let ctrl_down = (GetAsyncKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0;
        let shift_down = (GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;
        let alt_down = (GetAsyncKeyState(VK_MENU as i32) as u16 & 0x8000) != 0;
        let win_down = ((GetAsyncKeyState(VK_LWIN as i32) as u16 & 0x8000) != 0) 
                    || ((GetAsyncKeyState(VK_RWIN as i32) as u16 & 0x8000) != 0);

        req_ctrl == ctrl_down && req_shift == shift_down && req_alt == alt_down && req_win == win_down
    }
}
