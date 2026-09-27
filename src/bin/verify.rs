use std::io::Cursor;
use rodio::Decoder;

fn main() {
    let rain_bytes = include_bytes!("../../assets/asmr/rain.ogg");
    let rain_decode = Decoder::new(Cursor::new(rain_bytes));
    println!("rain.ogg decode: {:?}", rain_decode.is_ok());

    let wind_bytes = include_bytes!("../../assets/asmr/wind.ogg");
    let wind_decode = Decoder::new(Cursor::new(wind_bytes));
    println!("wind.ogg decode: {:?}", wind_decode.is_ok());

    let thunder_bytes = include_bytes!("../../assets/asmr/thunder.ogg");
    let thunder_decode = Decoder::new(Cursor::new(thunder_bytes));
    println!("thunder.ogg decode: {:?}", thunder_decode.is_ok());
}
