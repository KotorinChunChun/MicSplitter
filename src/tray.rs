use tray_icon::{TrayIcon, TrayIconBuilder, menu::{Menu, MenuItem, PredefinedMenuItem}};
use tray_icon::Icon;

pub struct TraySetup {
    pub tray_icon: TrayIcon,
    pub show_id: String,
    pub quit_id: String,
}

pub fn create_tray_icon() -> Result<TraySetup, Box<dyn std::error::Error>> {
    let tray_menu = Menu::new();
    let show_item = MenuItem::with_id("show", "Show", true, None);
    let quit_item = MenuItem::with_id("quit", "Quit", true, None);

    let show_id = show_item.id().clone().0;
    let quit_id = quit_item.id().clone().0;

    tray_menu.append_items(&[
        &show_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ])?;

    // 32x32のRGBAアイコンを生成 (赤色)
    let width = 32;
    let height = 32;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for _ in 0..(width * height) {
        rgba.push(255); // R
        rgba.push(100); // G
        rgba.push(100); // B
        rgba.push(255); // A
    }
    let icon = Icon::from_rgba(rgba, width, height)?;

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_menu_on_left_click(false)
        .with_tooltip("MicSplitter")
        .with_icon(icon)
        .build()?;

    Ok(TraySetup {
        tray_icon,
        show_id,
        quit_id,
    })
}
