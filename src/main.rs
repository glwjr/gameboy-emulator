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
        let interrupt_cycles = self.cpu.handle_interrupts(&mut self.bus);
        let cycles = self.cpu.step(&mut self.bus);
        self.bus.tick(interrupt_cycles + cycles);
    }

    fn pc(&self) -> u16 {
        self.cpu.pc()
    }

    fn print_trace(&self) {
        self.cpu.print_trace(&self.bus);
    }
}

fn main() -> Result<(), std::io::Error> {
    let path = "test_roms/cpu_instrs.gb";
    let rom = fs::read(path)?;

    let mut gameboy = GameBoy::new(rom);

    for _ in 0..10000000 {
        gameboy.step();
    }

    gameboy.print_trace();

    Ok(())
}
