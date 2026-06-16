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
    framebuffer: Vec<u32>,
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
            framebuffer: vec![0; 160 * 144],
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

    pub fn framebuffer(&self) -> &[u32] {
        &self.framebuffer
    }

    fn tile_pixel(&self, tile_addr: usize, row: u8, col: u8) -> u8 {
        let byte1 = self.vram[tile_addr + (row as usize) * 2];
        let byte2 = self.vram[tile_addr + (row as usize) * 2 + 1];
        let bit = 7 - col; // column 0 is the leftmost = bit 7
        let low = (byte1 >> bit) & 1;
        let high = (byte2 >> bit) & 1;
        (high << 1) | low
    }

    fn apply_palette(&self, color_id: u8) -> u32 {
        let shade = (self.bgp >> (color_id * 2)) & 0x03;
        match shade {
            0 => 0xFFFFFF, // white
            1 => 0xAAAAAA, // light gray
            2 => 0x555555, // dark gray
            3 => 0x000000, // black
            _ => unreachable!(),
        }
    }

    fn render_scanline(&mut self) {
        let ly = self.ly;
        if ly >= 144 {
            return;
        } // only visible lines

        // LCD off? leave blank.
        if self.lcdc & 0x80 == 0 {
            return;
        }

        // Tilemap base (LCDC bit 3) and tile-data mode (LCDC bit 4)
        let tilemap_base: usize = if self.lcdc & 0x08 != 0 {
            0x1C00
        } else {
            0x1800
        };
        let unsigned_tiles = self.lcdc & 0x10 != 0;

        for x in 0..160u8 {
            let bg_x = x.wrapping_add(self.scx);
            let bg_y = ly.wrapping_add(self.scy);

            let tile_col = (bg_x / 8) as usize;
            let tile_row = (bg_y / 8) as usize;
            let tile_index = tile_row * 32 + tile_col;
            let tile_num = self.vram[tilemap_base + tile_index];

            // Resolve tile data address (the LCDC bit-4 quirk)
            let tile_addr: usize = if unsigned_tiles {
                (tile_num as usize) * 16
            } else {
                // signed: base 0x1000 (=0x9000 in VRAM offset), index as i8
                (0x1000_i32 + (tile_num as i8 as i32) * 16) as usize
            };

            let row = bg_y % 8;
            let col = bg_x % 8;
            let color_id = self.tile_pixel(tile_addr, row, col);
            let color = self.apply_palette(color_id);

            self.framebuffer[ly as usize * 160 + x as usize] = color;
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
            self.render_scanline();

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
