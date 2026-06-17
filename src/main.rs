use minifb::{Key, Window, WindowOptions};
use std::fs;

mod bus;
mod cpu;
mod joypad;
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

    fn set_buttons(
        &mut self,
        right: bool,
        left: bool,
        up: bool,
        down: bool,
        a: bool,
        b: bool,
        select_btn: bool,
        start: bool,
    ) {
        self.bus
            .set_buttons(right, left, up, down, a, b, select_btn, start);
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
        let mut frame_cycles = 0u32;
        while frame_cycles < 70224 {
            frame_cycles += gameboy.step();
        }

        gameboy.set_buttons(
            window.is_key_down(Key::Right),
            window.is_key_down(Key::Left),
            window.is_key_down(Key::Up),
            window.is_key_down(Key::Down),
            window.is_key_down(Key::Z),         // A
            window.is_key_down(Key::X),         // B
            window.is_key_down(Key::Backspace), // Select
            window.is_key_down(Key::Enter),     // Start
        );

        window
            .update_with_buffer(gameboy.framebuffer(), WIDTH, HEIGHT)
            .unwrap();
    }

    Ok(())
}
