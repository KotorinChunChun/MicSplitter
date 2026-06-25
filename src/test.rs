use std::str::FromStr;
use global_hotkey::hotkey::HotKey;

fn main() {
    let hk = HotKey::from_str("Ctrl+Alt+Win+F9").unwrap();
    println!("Parsed: {:?}", hk);
}
