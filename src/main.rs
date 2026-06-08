use std::fs;

struct Cpu {}

impl Cpu {
    fn new() -> Self {
        Cpu {}
    }

    fn step(&mut self, bus: &mut Bus) -> u8 {
        let _first_byte = bus.read_byte(0x0100);
        0
    }
}

struct Bus {
    rom: Vec<u8>,
}

impl Bus {
    fn new(rom: Vec<u8>) -> Self {
        Bus { rom }
    }

    fn read_byte(&self, addr: u16) -> u8 {
        self.rom[addr as usize]
    }
}

struct GameBoy {
    cpu: Cpu,
    bus: Bus,
}

impl GameBoy {
    fn new(rom: Vec<u8>) -> Self {
        GameBoy {
            cpu: Cpu::new(),
            bus: Bus::new(rom),
        }
    }

    fn step(&mut self) {
        let _cycles = self.cpu.step(&mut self.bus);
    }
}

fn main() -> Result<(), std::io::Error> {
    let path = "roms/Legend of Zelda, The - Link's Awakening (USA, Europe).gb";
    let rom = fs::read(path)?;

    let mut gameboy = GameBoy::new(rom);
    gameboy.step();

    println!("one step ran");

    Ok(())
}
