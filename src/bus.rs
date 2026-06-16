use std::io::Write;

pub struct Bus {
    rom: Vec<u8>,
    rom_bank: u8,
    ram_enabled: bool,
    vram: [u8; 0x2000],
    cart_ram: [u8; 0x2000],
    wram: [u8; 0x2000],
    oam: [u8; 0xA0],
    io: [u8; 0x80],
    hram: [u8; 0x7F],
    ie: u8,
    scanline_cycles: u32,
    div_cycles: u16,
    tima_cycles: u32,
}

impl Bus {
    pub fn new(rom: Vec<u8>) -> Self {
        Bus {
            rom,
            rom_bank: 1,
            ram_enabled: false,
            vram: [0; 0x2000],
            cart_ram: [0; 0x2000],
            wram: [0; 0x2000],
            oam: [0; 0xA0],
            io: [0; 0x80],
            hram: [0; 0x7F],
            ie: 0,
            scanline_cycles: 0,
            div_cycles: 0,
            tima_cycles: 0,
        }
    }

    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => self.rom[addr as usize],
            0x4000..=0x7FFF => {
                let bank_base = self.rom_bank as usize * 0x4000;
                let offset = addr as usize - 0x4000;
                self.rom[bank_base + offset]
            }
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

    pub fn write_byte(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => {
                // RAM enable: 0x0A in the low nibble enables, anything else disables
                self.ram_enabled = (value & 0x0F) == 0x0A;
            }
            0x2000..=0x3FFF => {
                // ROM bank select (low 5 bits). Bank 0 remaps to 1.
                let bank = value & 0x1F;
                self.rom_bank = if bank == 0 { 1 } else { bank };
            }
            0x4000..=0x7FFF => {
                // Upper bits / mode -- unused for 512KB (32 banks fit in 5 bits).
            }
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize] = value,
            0xA000..=0xBFFF => self.cart_ram[(addr - 0xA000) as usize] = value,
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = value,
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize] = value,
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize] = value,
            0xFEA0..=0xFEFF => {}
            0xFF02 => {
                // SC, serial control -- writing 0x81 (bit 7 set) starts a transfer
                // Intercept it: emit the SB byte to stdout.
                if value & 0x80 != 0 {
                    let byte = self.io[0x01]; // SB, the data byte
                    print!("{}", byte as char);
                    std::io::stdout().flush().unwrap();
                }
                self.io[(addr - 0xFF00) as usize] = value & 0x7F; // clear start bit
            }
            0xFF04 => {
                // DIV -- any write resets it (and its accumulator) to zero
                self.io[0x04] = 0;
                self.div_cycles = 0;
            }
            0xFF00..=0xFF7F => self.io[(addr - 0xFF00) as usize] = value,
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = value,
            0xFFFF => self.ie = value,
        }
    }

    pub fn read_word(&self, addr: u16) -> u16 {
        let low = self.read_byte(addr) as u16;
        let high = self.read_byte(addr.wrapping_add(1)) as u16;
        low | (high << 8)
    }

    pub fn write_word(&mut self, addr: u16, value: u16) {
        self.write_byte(addr, value as u8);
        self.write_byte(addr.wrapping_add(1), (value >> 8) as u8);
    }

    pub fn tick(&mut self, cycles: u8) {
        self.scanline_cycles += cycles as u32;
        self.div_cycles += cycles as u16;

        // LY / VBlank
        if self.scanline_cycles >= 456 {
            self.scanline_cycles -= 456;
            let ly = self.io[0x44];
            let new_ly = if ly >= 153 { 0 } else { ly + 1 };
            self.io[0x44] = new_ly;
            if new_ly == 144 {
                self.io[0x0F] |= 0x01; // request VBlank interrupt (IF bit 0)
            }
        }

        // DIV: free-running, increments every 256 cycles
        if self.div_cycles >= 256 {
            self.div_cycles -= 256;
            self.io[0x04] = self.io[0x04].wrapping_add(1);
        }

        // TIMA: configurable counter, only when enabled (TAC bit 2)
        if self.io[0x07] & 0x04 != 0 {
            let threshold: u32 = match self.io[0x07] & 0x03 {
                0 => 1024, // 4096 Hz
                1 => 16,   // 262144 Hz
                2 => 64,   // 65536 Hz
                3 => 256,  // 16384 Hz
                _ => unreachable!(),
            };

            self.tima_cycles += cycles as u32;
            // while, not if: a single instruction can cross the threshold more
            // than once at the fastest (16-cycle) rate.
            while self.tima_cycles >= threshold {
                self.tima_cycles -= threshold;
                let (new_tima, overflow) = self.io[0x05].overflowing_add(1);
                if overflow {
                    // On overflow: reload from TMA (not 0) and request the
                    // timer interrupt (IF bit 2).
                    self.io[0x05] = self.io[0x06]; // TMA
                    self.io[0x0F] |= 0x04; // IF bit 2 = timer
                } else {
                    self.io[0x05] = new_tima;
                }
            }
        }
    }
}
