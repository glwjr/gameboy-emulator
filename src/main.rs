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
    vram: [u8; 0x2000],
    cart_ram: [u8; 0x2000],
    wram: [u8; 0x2000],
    oam: [u8; 0xA0],
    io: [u8; 0x80],
    hram: [u8; 0x7F],
    ie: u8,
}

impl Bus {
    fn new(rom: Vec<u8>) -> Self {
        Bus {
            rom,
            vram: [0; 0x2000],
            cart_ram: [0; 0x2000],
            wram: [0; 0x2000],
            oam: [0; 0xA0],
            io: [0; 0x80],
            hram: [0; 0x7F],
            ie: 0,
        }
    }

    fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.rom[addr as usize],
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize],
            0xA000..=0xBFFF => self.cart_ram[(addr - 0xA000) as usize],
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize],
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize],
            0xFEA0..=0xFEFF => 0xFF,
            0xFF00..=0xFF7F => self.io[(addr - 0xFF00) as usize],
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.ie,
        }
    }

    fn write_byte(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x7FFF => {}
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize] = value,
            0xA000..=0xBFFF => self.cart_ram[(addr - 0xA000) as usize] = value,
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = value,
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize] = value,
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize] = value,
            0xFEA0..=0xFEFF => {}
            0xFF00..=0xFF7F => self.io[(addr - 0xFF00) as usize] = value,
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = value,
            0xFFFF => self.ie = value,
        }
    }

    fn read_word(&self, addr: u16) -> u16 {
        let low = self.read_byte(addr) as u16;
        let high = self.read_byte(addr.wrapping_add(1)) as u16;
        low | (high << 8)
    }

    fn write_word(&mut self, addr: u16, value: u16) {
        self.write_byte(addr, value as u8);
        self.write_byte(addr.wrapping_add(1), (value >> 8) as u8);
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
    let bus = &mut gameboy.bus;

    bus.write_byte(0xC000, 0x42);
    bus.write_byte(0x8000, 0x37);
    bus.write_byte(0xFF80, 0x99);
    println!("WRAM 0xC000 = {:#04x} (expect 0x42)", bus.read_byte(0xC000));
    println!("VRAM 0x8000 = {:#04x} (expect 0x37)", bus.read_byte(0x8000));
    println!("HRAM 0xFF80 = {:#04x} (expect 0x99)", bus.read_byte(0xFF80));

    println!("ROM  0x0147 = {:#04x} (expect 0x03)", bus.read_byte(0x0147));

    bus.write_byte(0x0000, 0xFF);
    println!(
        "ROM  0x0000 = {:#04x} (unchanged by the write above)",
        bus.read_byte(0x0000)
    );

    bus.write_word(0xC010, 0xBEEF);
    println!(
        "word 0xC010 = {:#06x} (expect 0xbeef)",
        bus.read_word(0xC010)
    );
    println!(
        "  byte 0xC010 = {:#04x} (low, expect 0xef)",
        bus.read_byte(0xC010)
    );
    println!(
        "  byte 0xC011 = {:#04x} (high, expect 0xbe)",
        bus.read_byte(0xC011)
    );

    Ok(())
}
