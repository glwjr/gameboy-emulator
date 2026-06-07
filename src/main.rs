use std::fs;

fn main() -> Result<(), std::io::Error> {
    let path: &str = "roms/Legend of Zelda, The - Link's Awakening (USA, Europe).gb";
    let rom: Vec<u8> = fs::read(path)?;
    println!("Loaded {} bytes", rom.len());
    Ok(())
}
