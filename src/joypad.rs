pub struct Joypad {
    right: bool,
    left: bool,
    up: bool,
    down: bool,
    a: bool,
    b: bool,
    select_btn: bool,
    start: bool,
    select: u8,
}

impl Joypad {
    pub fn new() -> Self {
        Joypad {
            right: false,
            left: false,
            up: false,
            down: false,
            a: false,
            b: false,
            select_btn: false,
            start: false,
            select: 0,
        }
    }

    pub fn read(&self) -> u8 {
        let mut buttons = 0x0F; // default: all released (high)

        if self.select & 0x10 == 0 {
            // bit 4 low = D-pad selected
            if self.right {
                buttons &= !0x01;
            }
            if self.left {
                buttons &= !0x02;
            }
            if self.up {
                buttons &= !0x04;
            }
            if self.down {
                buttons &= !0x08;
            }
        }
        if self.select & 0x20 == 0 {
            // bit 5 low = action selected
            if self.a {
                buttons &= !0x01;
            }
            if self.b {
                buttons &= !0x02;
            }
            if self.select_btn {
                buttons &= !0x04;
            }
            if self.start {
                buttons &= !0x08;
            }
        }

        (self.select & 0x30) | buttons | 0xC0
    }

    pub fn write(&mut self, value: u8) {
        self.select = value & 0x30; // only bits 4-5 are writable
    }

    pub fn set_buttons(
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
        self.right = right;
        self.left = left;
        self.up = up;
        self.down = down;
        self.a = a;
        self.b = b;
        self.select_btn = select_btn;
        self.start = start;
    }
}
