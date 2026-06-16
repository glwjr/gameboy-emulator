use minifb::{Key, Window, WindowOptions};
use std::fs;

mod bus;
mod cpu;
mod ppu;

use bus::Bus;
use cpu::Cpu;

const WIDTH: usize = 160;
const HEIGHT: usize = 144;

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

    fn step(&mut self) -> u32 {
        let interrupt_cycles = self.cpu.handle_interrupts(&mut self.bus);
        let cycles = self.cpu.step(&mut self.bus);
        let total = interrupt_cycles + cycles;
        self.bus.tick(total);
        total as u32
    }

    fn framebuffer(&self) -> &[u32] {
        self.bus.framebuffer()
    }

    fn pc(&self) -> u16 {
        self.cpu.pc()
    }

    fn print_trace(&self) {
        self.cpu.print_trace(&self.bus);
    }
}

fn main() -> Result<(), std::io::Error> {
    let path = "roms/Legend of Zelda, The - Link's Awakening (USA, Europe).gb";
    let rom = fs::read(path)?;
    let mut gameboy = GameBoy::new(rom);

    let mut window = Window::new(
        "Game Boy",
        WIDTH,
        HEIGHT,
        WindowOptions {
            scale: minifb::Scale::X4, // 160x144 is tiny; scale up 4x
            ..WindowOptions::default()
        },
    )
    .unwrap();

    // ~60 fps cap
    window.set_target_fps(60);

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Run the emulator for roughly one frame's worth of cycles.
        // One frame = 70224 cycles. Step until we've covered that.
        let mut frame_cycles = 0u32;
        while frame_cycles < 70224 {
            frame_cycles += gameboy.step(); // step() that returns cycles
        }

        // Push the framebuffer to the window.
        window
            .update_with_buffer(gameboy.framebuffer(), WIDTH, HEIGHT)
            .unwrap();
    }

    Ok(())
}
