use rodio::{OutputStream, source::Source};
use rodio::cpal::traits::{DeviceTrait, HostTrait};

fn main() {
    let host = rodio::cpal::default_host();
    let device = host.default_output_device().unwrap();
    let default_config = device.default_output_config().unwrap();
    
    // How to pass it to rodio?
    // OutputStream::try_from_device(&device) exists? No.
}
