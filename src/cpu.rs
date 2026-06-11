use crate::bus::Bus;

const TRACE: bool = true;

const ZERO_FLAG: u8 = 0x80;
const SUBTRACT_FLAG: u8 = 0x40;
const HALF_CARRY_FLAG: u8 = 0x20;
const CARRY_FLAG: u8 = 0x10;

pub struct Cpu {
    a: u8,
    f: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    sp: u16,
    pc: u16,
    ime: bool,
    halted: bool,
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            a: 0x01,
            f: 0xB0,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
            ime: false,
            halted: false,
        }
    }

    fn get_zero_flag(&self) -> bool {
        (self.f & ZERO_FLAG) != 0
    }

    fn set_zero_flag(&mut self, on: bool) {
        if on {
            self.f |= ZERO_FLAG;
        } else {
            self.f &= !ZERO_FLAG;
        }
    }

    fn get_subtract_flag(&self) -> bool {
        (self.f & SUBTRACT_FLAG) != 0
    }

    fn set_subtract_flag(&mut self, on: bool) {
        if on {
            self.f |= SUBTRACT_FLAG;
        } else {
            self.f &= !SUBTRACT_FLAG;
        }
    }

    fn get_half_carry_flag(&self) -> bool {
        (self.f & HALF_CARRY_FLAG) != 0
    }

    fn set_half_carry_flag(&mut self, on: bool) {
        if on {
            self.f |= HALF_CARRY_FLAG;
        } else {
            self.f &= !HALF_CARRY_FLAG;
        }
    }

    fn get_carry_flag(&self) -> bool {
        (self.f & CARRY_FLAG) != 0
    }

    fn set_carry_flag(&mut self, on: bool) {
        if on {
            self.f |= CARRY_FLAG;
        } else {
            self.f &= !CARRY_FLAG;
        }
    }

    fn fetch_byte(&mut self, bus: &mut Bus) -> u8 {
        let byte = bus.read_byte(self.pc);
        self.pc = self.pc.wrapping_add(1);
        byte
    }

    fn fetch_word(&mut self, bus: &mut Bus) -> u16 {
        let low = self.fetch_byte(bus) as u16;
        let high = self.fetch_byte(bus) as u16;
        low | (high << 8)
    }

    fn push_word(&mut self, bus: &mut Bus, value: u16) {
        self.sp = self.sp.wrapping_sub(2);
        bus.write_word(self.sp, value);
    }

    pub fn step(&mut self, bus: &mut Bus) -> u8 {
        let pc = self.pc;
        if TRACE {
            println!(
                "A:{:02X} F:{:02X} B:{:02X} C:{:02X} D:{:02X} E:{:02X} H:{:02X} L:{:02X} SP:{:04X} PC:{:04X} PCMEM:{:02X},{:02X},{:02X},{:02X}",
                self.a,
                self.f,
                self.b,
                self.c,
                self.d,
                self.e,
                self.h,
                self.l,
                self.sp,
                self.pc,
                bus.read_byte(self.pc),
                bus.read_byte(self.pc.wrapping_add(1)),
                bus.read_byte(self.pc.wrapping_add(2)),
                bus.read_byte(self.pc.wrapping_add(3)),
            )
        }
        let opcode = self.fetch_byte(bus);

        match opcode {
            0x00 => 4, // NOP
            0xC3 => {
                // JP nn
                let addr = self.fetch_word(bus);
                self.pc = addr;
                16
            }
            0xCD => {
                // CALL nn
                let target = self.fetch_word(bus);
                self.push_word(bus, self.pc);
                self.pc = target;
                24
            }
            0xF0 => {
                // LDH A, (n) - load into A from the high page
                let n = self.fetch_byte(bus);
                let addr = 0xFF00 | (n as u16);
                self.a = bus.read_byte(addr);
                12
            }
            0xE0 => {
                // LDH (n), A - store from A to the high page
                let n = self.fetch_byte(bus);
                let addr = 0xFF00 | (n as u16);
                bus.write_byte(addr, self.a);
                12
            }
            0xCB => self.step_cb(bus), // hand off to the CB table
            0xFE => {
                // CP n
                let n = self.fetch_byte(bus);
                let a = self.a;
                self.set_zero_flag(a == n);
                self.set_subtract_flag(true);
                self.set_half_carry_flag((a & 0x0F) < (n & 0x0F));
                self.set_carry_flag(a < n);
                8
            }
            0x20 => {
                // JR NZ, e
                // if Z is clear, hop forward or backward by e bytes
                // otherwise fall through to the next instruction
                let offset = self.fetch_byte(bus) as i8;
                if !self.get_zero_flag() {
                    self.pc = self.pc.wrapping_add(offset as u16); // taken
                    12
                } else {
                    8 // not taken: just fall through
                }
            }
            _ => panic!("unimplemented opcode {:#04x} at {:#06x}", opcode, pc),
        }
    }

    fn step_cb(&mut self, bus: &mut Bus) -> u8 {
        let cb_opcode = self.fetch_byte(bus);
        match cb_opcode {
            0x87 => {
                // RES 0, A
                self.a &= !0x01;
                8
            }
            _ => panic!("unimplemented CB opcode {:#04x}", cb_opcode),
        }
    }

    fn get_af(&self) -> u16 {
        (self.a as u16) << 8 | self.f as u16
    }

    fn set_af(&mut self, value: u16) {
        self.a = (value >> 8) as u8;
        self.f = (value as u8) & 0xF0;
    }

    fn get_bc(&self) -> u16 {
        (self.b as u16) << 8 | self.c as u16
    }

    fn set_bc(&mut self, value: u16) {
        self.b = (value >> 8) as u8;
        self.c = value as u8;
    }

    fn get_de(&self) -> u16 {
        (self.d as u16) << 8 | self.e as u16
    }

    fn set_de(&mut self, value: u16) {
        self.d = (value >> 8) as u8;
        self.e = value as u8;
    }

    fn get_hl(&self) -> u16 {
        (self.h as u16) << 8 | self.l as u16
    }

    fn set_hl(&mut self, value: u16) {
        self.h = (value >> 8) as u8;
        self.l = value as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(program: &[u8]) -> (Cpu, Bus) {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new(vec![0; 0x8000]);
        cpu.pc = 0xC000;
        for (i, &byte) in program.iter().enumerate() {
            bus.write_byte(0xC000 + i as u16, byte);
        }
        (cpu, bus)
    }

    #[test]
    fn nop_advances_pc_and_touches_nothing_else() {
        let (mut cpu, mut bus) = setup(&[0x00]); // NOP

        let af_before = cpu.get_af();
        let bc_before = cpu.get_bc();
        let sp_before = cpu.sp;

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 4, "NOP should take 4 cycles");
        assert_eq!(cpu.pc, 0xC001, "NOP should advance pc by exactly 1");
        assert_eq!(cpu.get_af(), af_before, "NOP must not touch A or flags");
        assert_eq!(cpu.get_bc(), bc_before, "NOP must not touch BC");
        assert_eq!(cpu.sp, sp_before, "NOP must not touch sp");
    }

    #[test]
    fn call_pushes_return_address_and_jumps() {
        let (mut cpu, mut bus) = setup(&[0xCD, 0x34, 0x12]);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cpu.pc, 0x1234, "CALL should jump to the operand address");
        assert_eq!(cycles, 24, "CALL nn should take 24 cycles");
        assert_eq!(cpu.sp, 0xFFFC, "CALL should push 2 bytes (sp -= 2)");
        assert_eq!(
            bus.read_word(cpu.sp),
            0xC003,
            "CALL should push the address of the next instruction"
        );
        assert_eq!(bus.read_byte(0xFFFC), 0x03, "low byte of return address");
        assert_eq!(bus.read_byte(0xFFFD), 0xC0, "high byte of return address");
    }

    #[test]
    fn cp_equal_sets_zero_clears_carry() {
        // A == n
        // Result is zero, no borrow anywhere
        let (mut cpu, mut bus) = setup(&[0xFE, 0x42]); // CP 0x42
        cpu.a = 0x42;

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 8, "CP n should take 8 cycles");
        assert_eq!(cpu.a, 0x42, "CP must not modify A");
        assert!(cpu.get_zero_flag(), "A == n should set Z");
        assert!(cpu.get_subtract_flag(), "CP always sets N");
        assert!(!cpu.get_half_carry_flag(), "equal values: no half-borrow");
        assert!(!cpu.get_carry_flag(), "equal values: no full borrow");
    }

    #[test]
    fn cp_smaller_a_sets_carry_and_half_carry() {
        // A < n with a low-nibble borrow
        // 0x00 - 0x91
        let (mut cpu, mut bus) = setup(&[0xFE, 0x91]); // CP 0x91
        cpu.a = 0x00;

        cpu.step(&mut bus);

        assert_eq!(cpu.a, 0x00, "CP must not modify A");
        assert!(!cpu.get_zero_flag(), "0x00 != 0x91 should clear Z");
        assert!(cpu.get_subtract_flag(), "CP always sets N");
        assert!(
            cpu.get_half_carry_flag(),
            "low nibble 0x0 < 0x1 should set H"
        );
        assert!(cpu.get_carry_flag(), "0x00 < 0x91 should set C");
    }

    #[test]
    fn cp_half_carry_without_full_carry() {
        // A > n overall (no full borrow), but the low
        // nibble still borrows: 0x10 - 0x01
        let (mut cpu, mut bus) = setup(&[0xFE, 0x01]); // CP 0x01
        cpu.a = 0x10;

        cpu.step(&mut bus);

        assert_eq!(cpu.a, 0x10, "CP must not modify A");
        assert!(!cpu.get_zero_flag(), "0x10 != 0x01 should clear Z");
        assert!(cpu.get_subtract_flag(), "CP always sets N");
        assert!(
            cpu.get_half_carry_flag(),
            "low nibble 0x0 < 0x1 sets H even though A > n"
        );
        assert!(
            !cpu.get_carry_flag(),
            "0x10 > 0x01 overall, so no full borrow: C clear"
        );
    }

    #[test]
    fn jr_nz_taken_jumps_backward() {
        // Z clear -> branch taken
        // Offset 0xFA = -6 (signed)
        // After fetching the 2-byte instruction, pc = 0xC002
        // Applying -6:  0xC002 - 6 = 0xBFFC
        let (mut cpu, mut bus) = setup(&[0x20, 0xFA]); // JR NZ, -6
        cpu.set_zero_flag(false);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 12, "taken JR should cost 12 cycles");
        assert_eq!(
            cpu.pc, 0xBFFC,
            "taken backward jump: 0xC002 + (-6) = 0xBFFC"
        );
    }

    #[test]
    fn jr_nz_taken_jumps_forward() {
        // Z clear -> taken
        // Offset 0x05 = +5 (signed)
        // After fetch pc = 0xC002, applying +5 -> 0xC007
        let (mut cpu, mut bus) = setup(&[0x20, 0x05]); // JR NZ, +5
        cpu.set_zero_flag(false);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 12, "taken JR should cost 12 cycles");
        assert_eq!(cpu.pc, 0xC007, "taken forward jump: 0xC002 + 5 = 0xC007");
    }

    #[test]
    fn jr_nz_not_taken_falls_through() {
        // Z set -> branch not taken
        // The offset byte is still consumed,
        // so pc must advance past the full 2-byte instruction to 0xC002,
        // and the offset must not be applied
        let (mut cpu, mut bus) = setup(&[0x20, 0xFA]); // JR NZ, -6 (ignored)
        cpu.set_zero_flag(true);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 8, "not-taken JR should cost 8 cycles");
        assert_eq!(
            cpu.pc, 0xC002,
            "not taken: pc advances past both bytes, offset not applied"
        );
    }
}
