use global_hotkey::{GlobalHotKeyManager, hotkey::{HotKey, Modifiers, Code}};

#[allow(dead_code)]
pub struct Hotkeys {
    pub manager: GlobalHotKeyManager,
    pub toggle_mon_id: u32,
    pub toggle_out1_id: u32,
    pub toggle_out2_id: u32,
    pub toggle_in_id: u32,
    pub toggle_swap_id: u32,
}

pub fn register_hotkeys() -> Result<Hotkeys, Box<dyn std::error::Error>> {
    let manager = GlobalHotKeyManager::new()?;

    // Modifiers::SUPER が Windowsキー に該当します
    let mods = Some(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER);

    // Monitor: Ctrl + Alt + Win + F8
    let toggle_mon = HotKey::new(mods, Code::F8);
    // Output 1: Ctrl + Alt + Win + F9
    let toggle_out1 = HotKey::new(mods, Code::F9);
    // Output 2: Ctrl + Alt + Win + F10
    let toggle_out2 = HotKey::new(mods, Code::F10);
    // Input: Ctrl + Alt + Win + F7
    let toggle_in = HotKey::new(mods, Code::F7);
    // Swap 1 and 2: Ctrl + Alt + Win + F11
    let toggle_swap = HotKey::new(mods, Code::F11);

    let toggle_mon_id = toggle_mon.id();
    let toggle_out1_id = toggle_out1.id();
    let toggle_out2_id = toggle_out2.id();
    let toggle_in_id = toggle_in.id();
    let toggle_swap_id = toggle_swap.id();

    manager.register(toggle_mon)?;
    manager.register(toggle_out1)?;
    manager.register(toggle_out2)?;
    manager.register(toggle_in)?;
    manager.register(toggle_swap)?;

    Ok(Hotkeys {
        manager,
        toggle_mon_id,
        toggle_out1_id,
        toggle_out2_id,
        toggle_in_id,
        toggle_swap_id,
    })
}
