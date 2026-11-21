// Proves DDC/CI against the monitors actually attached. Reads only; never writes.
use ddc_hi::{Ddc, Display};

fn main() {
    let mut displays = Display::enumerate();
    println!("displays enumerated: {}", displays.len());
    for d in &mut displays {
        println!("--- id={} info={:?}", d.info.id, d.info);
        for (name, code) in [("brightness", 0x10u8), ("contrast", 0x12), ("volume", 0x62)] {
            match d.handle.get_vcp_feature(code) {
                Ok(v) => println!("  {name} 0x{code:02x} cur={} max={}", v.value(), v.maximum()),
                Err(e) => println!("  {name} 0x{code:02x} ERR {e}"),
            }
        }
    }
}
