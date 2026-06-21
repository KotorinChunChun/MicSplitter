use cpal::traits::{DeviceTrait, HostTrait};

fn main() {
    let host = cpal::default_host();
    if let Some(dev) = host.default_input_device() {
        println!("{}", dev.name().unwrap());
    }
}
