use std::fs;

mod bus;
mod cpu;

use bus::Bus;
use cpu::Cpu;

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
        let cycles = self.cpu.step(&mut self.bus);
        self.bus.tick(cycles);
    }
}

fn main() -> Result<(), std::io::Error> {
    let path = "roms/Legend of Zelda, The - Link's Awakening (USA, Europe).gb";
    let rom = fs::read(path)?;

    let mut gameboy = GameBoy::new(rom);

    for _ in 0..100000 {
        gameboy.step();
    }

    Ok(())
}
