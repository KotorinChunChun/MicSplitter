use tray_icon::{TrayIcon, TrayIconBuilder, menu::{Menu, MenuItem, PredefinedMenuItem}};
use tray_icon::Icon;

pub struct TraySetup {
    pub tray_icon: TrayIcon,
    pub show_id: String,
    pub quit_id: String,
}

pub fn generate_icon(in_enabled: bool, out1: bool, out2: bool, cfg: &crate::config::Config) -> Icon {
    let width = 32;
    let height = 32;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..height {
        for x in 0..width {
            let [mut r, mut g, mut b] = match (out1, out2) {
                (true, true) => {
                    if x + y < 32 {
                        cfg.icon_color_out1_on
                    } else {
                        cfg.icon_color_out2_on
                    }
                },
                (true, false) => cfg.icon_color_out1_on,
                (false, true) => cfg.icon_color_out2_on,
                (false, false) => cfg.icon_color_both_off,
            };

            if !in_enabled {
                let is_cross = (x as i32 - y as i32).abs() <= 2 || (x as i32 + y as i32 - 31).abs() <= 2;
                let in_bounds = x > 6 && x < 26 && y > 6 && y < 26;
                if is_cross && in_bounds {
                    r = 255;
                    g = 0;
                    b = 0;
                }
            }

            rgba.push(r);
            rgba.push(g);
            rgba.push(b);
            rgba.push(255); // A
        }
    }
    Icon::from_rgba(rgba, width as u32, height as u32).unwrap()
}

pub fn update_tray_icon(tray_icon: &TrayIcon, in_enabled: bool, out1: bool, out2: bool, cfg: &crate::config::Config) {
    let icon = generate_icon(in_enabled, out1, out2, cfg);
    let _ = tray_icon.set_icon(Some(icon));
}

pub fn create_tray_icon(initial_in: bool, initial_out1: bool, initial_out2: bool, cfg: &crate::config::Config) -> Result<TraySetup, Box<dyn std::error::Error>> {
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

    let icon = generate_icon(initial_in, initial_out1, initial_out2, cfg);

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
