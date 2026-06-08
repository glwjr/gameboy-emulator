use std::fs;

fn main() -> Result<(), std::io::Error> {
    let path = "roms/Legend of Zelda, The - Link's Awakening (USA, Europe).gb";
    let rom = fs::read(path)?;

    let cart_type = rom[0x147];
    let mbc_name = match cart_type {
        0x00 => "ROM ONLY",
        0x01 => "MBC1",
        0x03 => "MBC1+RAM+BATTERY",
        0x13 => "MBC3+RAM+BATTERY",
        _ => "UNKNOWN",
    };

    let title = &rom[0x134..=0x143];
    let title = String::from_utf8_lossy(title);
    let title = title.trim_end_matches('\0');

    println!("title: {title}, cartridge type: {cart_type:#04x} -> {mbc_name}");

    Ok(())
}
