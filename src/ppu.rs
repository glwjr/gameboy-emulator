pub struct Ppu {
    vram: [u8; 0x2000],
    oam: [u8; 0xA0],

    // Registers
    lcdc: u8, // 0xFF40
    stat: u8, // 0xFF41
    scy: u8,  // 0xFF42
    scx: u8,  // 0xFF43
    ly: u8,   // 0xFF44
    lyc: u8,  // 0xFF45
    bgp: u8,  // 0xFF47
    obp0: u8, // 0xFF48
    obp1: u8, // 0xFF49
    wy: u8,   // 0xFF4A
    wx: u8,   // 0xFF4B

    scanline_cycles: u32,
}

pub struct InterruptRequest {
    pub vblank: bool,
    pub stat: bool,
}

impl Ppu {
    pub fn new() -> Self {
        Ppu {
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            lcdc: 0,
            stat: 0,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0,
            obp0: 0,
            obp1: 0,
            wy: 0,
            wx: 0,
            scanline_cycles: 0,
        }
    }

    pub fn read_vram(&self, addr: u16) -> u8 {
        self.vram[(addr - 0x8000) as usize]
    }

    pub fn write_vram(&mut self, addr: u16, value: u8) {
        self.vram[(addr - 0x8000) as usize] = value;
    }

    pub fn read_oam(&self, addr: u16) -> u8 {
        self.oam[(addr - 0xFE00) as usize]
    }

    pub fn write_oam(&mut self, addr: u16, value: u8) {
        self.oam[(addr - 0xFE00) as usize] = value;
    }

    pub fn read_register(&self, addr: u16) -> u8 {
        match addr {
            0xFF40 => self.lcdc,
            0xFF41 => self.stat,
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.ly,
            0xFF45 => self.lyc,
            0xFF47 => self.bgp,
            0xFF48 => self.obp0,
            0xFF49 => self.obp1,
            0xFF4A => self.wy,
            0xFF4B => self.wx,
            _ => 0xFF, // unmapped PPU register
        }
    }

    pub fn write_register(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF40 => self.lcdc = value,
            0xFF41 => self.stat = value,
            0xFF42 => self.scy = value,
            0xFF43 => self.scx = value,
            0xFF44 => {} // LY is read-only -- writes ignored
            0xFF45 => self.lyc = value,
            0xFF47 => self.bgp = value,
            0xFF48 => self.obp0 = value,
            0xFF49 => self.obp1 = value,
            0xFF4A => self.wy = value,
            0xFF4B => self.wx = value,
            _ => {}
        }
    }

    pub fn tick(&mut self, cycles: u8) -> InterruptRequest {
        let mut interrupts = InterruptRequest {
            vblank: false,
            stat: false,
        };

        // Mode in STAT's low 2 bits (computed from current LY + cycle position)
        let mode: u8 = if self.ly >= 144 {
            1 // VBlank
        } else {
            match self.scanline_cycles {
                0..=79 => 2,
                80..=251 => 3,
                _ => 0,
            }
        };
        self.stat = (self.stat & !0x03) | mode;

        self.scanline_cycles += cycles as u32;

        if self.scanline_cycles >= 456 {
            self.scanline_cycles -= 456;
            self.ly = if self.ly >= 153 { 0 } else { self.ly + 1 };

            if self.ly == 144 {
                interrupts.vblank = true;
            }

            // LY == LYC coincidence
            if self.ly == self.lyc {
                self.stat |= 0x04;
                if self.stat & 0x40 != 0 {
                    interrupts.stat = true;
                }
            } else {
                self.stat &= !0x04;
            }
        }

        interrupts
    }
}
