use crate::bus::Bus;

const TRACE: bool = false;

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
        // Post-boot register state (DMG)
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

    // Public API

    pub fn step(&mut self, bus: &mut Bus) -> u8 {
        // Trace must fire before the fetch: the Doctor format reports the
        // state at the instruction boundary (PC = this instruction's
        // address, PCMEM = its bytes). After the fetch, pc has advanced.
        if TRACE {
            self.print_trace(bus);
        }

        let pc = self.pc;
        let opcode = self.fetch_byte(bus);

        match opcode {
            0x00 => 4, // NOP
            0x01 => {
                // LD BC, nn - load a 16-bit immediate into BC
                let nn = self.fetch_word(bus);
                self.set_bc(nn);
                12
            }
            0x06 => {
                // LD B, n
                self.b = self.fetch_byte(bus);
                8
            }
            0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
                let dst = (opcode >> 3) & 0x07;
                let v = self.read_r8(bus, dst);
                let r = self.inc_r8(v);
                self.write_r8(bus, dst, r);
                if dst == 6 { 12 } else { 4 }
            }
            0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
                let dst = (opcode >> 3) & 0x07;
                let v = self.read_r8(bus, dst);
                let r = self.dec_r8(v);
                self.write_r8(bus, dst, r);
                if dst == 6 { 12 } else { 4 }
            }
            0x0B => {
                // DEC BC
                // Decrement the BC register pair -- no flags
                let bc = self.get_bc();
                self.set_bc(bc.wrapping_sub(1));
                8
            }
            0x0E => {
                // LD C, n - load an immediate byte into C
                self.c = self.fetch_byte(bus);
                8
            }
            0x11 => {
                // LD DE, nn
                let nn = self.fetch_word(bus);
                self.set_de(nn);
                12
            }
            0x12 => {
                // LD (DE), A
                let addr = self.get_de();
                bus.write_byte(addr, self.a);
                8
            }
            0x13 => {
                // INC DE -- no flags
                let de = self.get_de();
                self.set_de(de.wrapping_add(1));
                8
            }
            0x19 => {
                // ADD HL, DE
                let hl = self.get_hl();
                let de = self.get_de();
                self.set_hl(hl.wrapping_add(de));
                // Z is preserved -- ADD HL, rr never touches it
                self.set_subtract_flag(false);
                self.set_half_carry_flag((hl & 0x0FFF) + (de & 0x0FFF) > 0x0FFF);
                self.set_carry_flag(hl as u32 + de as u32 > 0xFFFF);
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
            0x21 => {
                // LD HL, nn - load a 16-bit immediate into HL
                let nn = self.fetch_word(bus);
                self.set_hl(nn);
                12
            }
            0x22 => {
                // LD (HL+), A - store A into memory at address HL, then increment HL
                let addr = self.get_hl();
                bus.write_byte(addr, self.a);
                self.set_hl(addr.wrapping_add(1));
                8
            }
            0x2A => {
                // LD A, (HL+) - read from memory at HL into A, then increment HL
                let addr = self.get_hl();
                self.a = bus.read_byte(addr);
                self.set_hl(addr.wrapping_add(1));
                8
            }
            0x31 => {
                // LD SP, nn - load a 16-bit immediate into the stack pointer
                self.sp = self.fetch_word(bus);
                12
            }
            0x3E => {
                // LD A, n - load an immediate byte into A
                self.a = self.fetch_byte(bus);
                8
            }
            0x36 => {
                // LD (HL), n
                let n = self.fetch_byte(bus);
                let addr = self.get_hl();
                bus.write_byte(addr, n);
                12
            }
            0x76 => panic!("HALT not yet implemented"), // must precede 0x40..=0x7F
            0x40..=0x7F => {
                let dst = (opcode >> 3) & 0x07;
                let src = opcode & 0x07;
                let value = self.read_r8(bus, src);
                self.write_r8(bus, dst, value);

                if dst == 6 || src == 6 { 8 } else { 4 }
            }
            0xAF => {
                // XOR A
                // XOR the A register with itself
                self.a ^= self.a;
                self.set_zero_flag(self.a == 0);
                self.set_subtract_flag(false);
                self.set_half_carry_flag(false);
                self.set_carry_flag(false);
                4
            }
            0xB1 => {
                // OR C
                self.a |= self.c;
                self.set_zero_flag(self.a == 0);
                self.set_subtract_flag(false);
                self.set_half_carry_flag(false);
                self.set_carry_flag(false);
                4
            }
            0xB8..=0xBF => {
                let src = opcode & 0x07;
                let v = self.read_r8(bus, src);
                self.alu_cp(v);
                if src == 6 { 8 } else { 4 }
            }
            0xC3 => {
                // JP nn
                let addr = self.fetch_word(bus);
                self.pc = addr;
                16
            }
            0xC9 => {
                // RET
                self.pc = self.pop_word(bus);
                16
            }
            0xCB => self.step_cb(bus), // hand off to the CB table
            0xCD => {
                // CALL nn
                let target = self.fetch_word(bus);
                self.push_word(bus, self.pc);
                self.pc = target;
                24
            }
            0xE0 => {
                // LDH (n), A - store from A to the high page
                let n = self.fetch_byte(bus);
                let addr = 0xFF00 | (n as u16);
                bus.write_byte(addr, self.a);
                12
            }
            0xE1 => {
                // POP HL
                let value = self.pop_word(bus);
                self.set_hl(value);
                12
            }
            0xE2 => {
                // LD (C), A - store A at 0xFF00 plus C
                let addr = 0xFF00 | (self.c as u16);
                bus.write_byte(addr, self.a);
                8
            }
            0xE5 => {
                // PUSH HL
                let value = self.get_hl();
                self.push_word(bus, value);
                16
            }
            0xE6 => {
                // AND n
                let n = self.fetch_byte(bus);
                self.a &= n;
                self.set_zero_flag(self.a == 0);
                self.set_subtract_flag(false);
                self.set_half_carry_flag(true);
                self.set_carry_flag(false);
                8
            }
            0xEA => {
                // LD (nn), A
                // Store A into memory at a 16-bit immediate address
                let addr = self.fetch_word(bus);
                bus.write_byte(addr, self.a);
                16
            }
            0xF0 => {
                // LDH A, (n) - load into A from the high page
                let n = self.fetch_byte(bus);
                let addr = 0xFF00 | (n as u16);
                self.a = bus.read_byte(addr);
                12
            }
            0xFE => {
                // CP n
                let n = self.fetch_byte(bus);
                self.alu_cp(n);
                8
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

    pub fn print_trace(&self, bus: &Bus) {
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

    pub fn pc(&self) -> u16 {
        self.pc
    }

    // Fetch and stack helpers

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

    fn pop_word(&mut self, bus: &mut Bus) -> u16 {
        let value = bus.read_word(self.sp);
        self.sp = self.sp.wrapping_add(2);
        value
    }

    fn read_r8(&mut self, bus: &mut Bus, index: u8) -> u8 {
        // index order: B C D E H L (HL) A -- fixed by the opcode encoding
        match index {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => self.h,
            5 => self.l,
            6 => bus.read_byte(self.get_hl()),
            7 => self.a,
            _ => panic!("invalid r8 index: {}", index),
        }
    }

    fn write_r8(&mut self, bus: &mut Bus, index: u8, value: u8) {
        // index order: B C D E H L (HL) A -- fixed by the opcode encoding
        match index {
            0 => self.b = value,
            1 => self.c = value,
            2 => self.d = value,
            3 => self.e = value,
            4 => self.h = value,
            5 => self.l = value,
            6 => bus.write_byte(self.get_hl(), value),
            7 => self.a = value,
            _ => panic!("invalid r8 index: {}", index),
        }
    }

    fn inc_r8(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);
        self.set_zero_flag(result == 0);
        self.set_subtract_flag(false);
        self.set_half_carry_flag((value & 0x0F) == 0x0F);
        // Carry is preserved -- INC never touches it
        result
    }

    fn dec_r8(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);
        self.set_zero_flag(result == 0);
        self.set_subtract_flag(true);
        self.set_half_carry_flag((value & 0x0F) == 0x00);
        // Carry is preserved -- DEC never touches it
        result
    }

    // ALU collapse

    fn alu_cp(&mut self, value: u8) {
        let a = self.a;
        self.set_zero_flag(a == value);
        self.set_subtract_flag(true);
        self.set_half_carry_flag((a & 0x0F) < (value & 0x0F));
        self.set_carry_flag(a < value);
    }

    // Flag accessors (F register, high nibble only)

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

    // Register pair accessors

    fn get_af(&self) -> u16 {
        (self.a as u16) << 8 | self.f as u16
    }

    fn set_af(&mut self, value: u16) {
        self.a = (value >> 8) as u8;
        self.f = (value as u8) & 0xF0; // F low nibble is always zero
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

    // read_r8 / write_r8 decoders

    #[test]
    fn r8_decoders_map_indices_to_registers_and_memory() {
        // Index order: B C D E H L (HL) A -- the directory both helpers share
        // Distinct sentinels per register so a transposed entry can't pass
        let (mut cpu, mut bus) = setup(&[]); // no program -- calling helpers directly

        // -- read_r8: spot-check ends and middle of the directory
        cpu.b = 0x11;
        cpu.e = 0x33;
        cpu.a = 0x77;
        assert_eq!(cpu.read_r8(&mut bus, 0), 0x11, "index 0 is B");
        assert_eq!(cpu.read_r8(&mut bus, 3), 0x33, "index 3 is E");
        assert_eq!(cpu.read_r8(&mut bus, 7), 0x77, "index 7 is A");

        // -- write_r8 -> read_r8 round trip through a plain register
        cpu.write_r8(&mut bus, 4, 0x44);
        assert_eq!(cpu.h, 0x44, "index 4 writes land in H");
        assert_eq!(cpu.read_r8(&mut bus, 4), 0x44, "index 4 round-trips");

        // -- index 6: (HL) must go through MEMORY, not any register
        cpu.set_hl(0xC050);
        cpu.write_r8(&mut bus, 6, 0x99);
        assert_eq!(
            bus.read_byte(0xC050),
            0x99,
            "index 6 write must land in memory at HL"
        );
        assert_eq!(
            cpu.read_r8(&mut bus, 6),
            0x99,
            "index 6 read must come from memory at HL"
        );
    }

    // 0x00 NOP

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

    // 0x05 DEC B

    #[test]
    fn dec_b_sets_hn_clears_z_and_preserves_carry() {
        let (mut cpu, mut bus) = setup(&[0x05]); // DEC B
        cpu.b = 0x10;
        cpu.set_carry_flag(true);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 4, "DEC B should take 4 cycles");
        assert_eq!(cpu.b, 0x0F, "decrementing 0x10 gives 0x0F -- nonzero");
        assert!(!cpu.get_zero_flag(), "nonzero result: Z clear");
        assert!(cpu.get_subtract_flag(), "DEC always sets N");
        assert!(cpu.get_half_carry_flag(), "low nibble 0x0 borrow: H sets");
        assert!(
            cpu.get_carry_flag(),
            "DEC must preserve carry -- not clear it"
        );
    }

    #[test]
    fn dec_b_sets_z_clears_h_and_preserves_carry() {
        let (mut cpu, mut bus) = setup(&[0x05]); // DEC B
        cpu.b = 0x01;
        cpu.set_carry_flag(true);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 4, "DEC B should take 4 cycles");
        assert_eq!(cpu.b, 0x00, "decrementing 0x01 gives 0x00");
        assert!(cpu.get_zero_flag(), "result is zero: Z is set");
        assert!(cpu.get_subtract_flag(), "DEC always sets N");
        assert!(
            !cpu.get_half_carry_flag(),
            "low nibble 0x01 absorbs the decrement: no borrow, H clear"
        );
        assert!(
            cpu.get_carry_flag(),
            "DEC must preserve carry -- not clear it"
        );
    }

    // 0x0C INC C

    #[test]
    fn inc_c_half_carry_and_preserves_carry() {
        let (mut cpu, mut bus) = setup(&[0x0C]); // INC C
        cpu.c = 0x0F;
        cpu.set_carry_flag(true);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 4, "INC C should take 4 cycles");
        assert_eq!(cpu.c, 0x10, "incrementing 0x0F gives 0x10 -- nonzero");
        assert!(!cpu.get_zero_flag(), "nonzero result: Z clear");
        assert!(!cpu.get_subtract_flag(), "INC always clears N");
        assert!(cpu.get_half_carry_flag(), "low nibble 0xF overflow: H sets");
        assert!(
            cpu.get_carry_flag(),
            "INC must preserve carry -- not clear it"
        );
    }

    #[test]
    fn inc_c_wraps_to_zero_and_sets_z() {
        let (mut cpu, mut bus) = setup(&[0x0C]); // INC C
        cpu.c = 0xFF;
        cpu.set_carry_flag(false);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 4, "INC C should take 4 cycles");
        assert_eq!(cpu.c, 0x00, "0xFF + 1 wraps to 0x00");
        assert!(cpu.get_zero_flag(), "zero result: Z set");
        assert!(!cpu.get_subtract_flag(), "INC always clears N");
        assert!(cpu.get_half_carry_flag(), "low nibble was 0xF: H sets");
        assert!(
            !cpu.get_carry_flag(),
            "INC must preserve carry -- false stays false"
        );
    }

    // 0x19 ADD HL, DE

    #[test]
    fn add_hl_de_no_carries() {
        let (mut cpu, mut bus) = setup(&[0x19]); // ADD HL, DE
        cpu.set_hl(0x1234);
        cpu.set_de(0x0111);
        cpu.set_zero_flag(true);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 8, "ADD HL should take 8 cycles");
        assert_eq!(cpu.get_hl(), 0x1345, "0x1234 + 0x0111 lands in HL");
        assert!(cpu.get_zero_flag(), "Z is preserved");
        assert!(!cpu.get_subtract_flag(), "ADD clears N");
        assert!(!cpu.get_half_carry_flag(), "no bit-11 carry: H clear");
        assert!(!cpu.get_carry_flag(), "no bit-15 carry: C clear");
    }

    #[test]
    fn add_hl_de_half_carry_without_carry() {
        let (mut cpu, mut bus) = setup(&[0x19]); // ADD HL, DE
        cpu.set_hl(0x0FFF);
        cpu.set_de(0x0001);
        cpu.set_zero_flag(true);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 8, "ADD HL, DE should take 8 cycles");
        assert_eq!(cpu.get_hl(), 0x1000, "0x0FFF + 0x0001 lands in HL");
        assert!(cpu.get_zero_flag(), "Z is preserved");
        assert!(!cpu.get_subtract_flag(), "ADD HL, rr clears N");
        assert!(cpu.get_half_carry_flag(), "bit-11 carry: H sets");
        assert!(!cpu.get_carry_flag(), "no bit-15 carry: C clear");
    }

    #[test]
    fn add_hl_de_wraps_and_preserves_z() {
        let (mut cpu, mut bus) = setup(&[0x19]); // ADD HL, DE
        cpu.set_hl(0xFFFF);
        cpu.set_de(0x0001);
        cpu.set_zero_flag(false);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 8, "ADD HL, DE should take 8 cycles");
        assert_eq!(cpu.get_hl(), 0x0000, "0xFFFF + 0x0001 wraps to 0x0000");
        assert!(
            !cpu.get_zero_flag(),
            "result is zero but Z must NOT set -- ADD HL, rr never computes Z"
        );
        assert!(!cpu.get_subtract_flag(), "ADD HL, rr clears N");
        assert!(
            cpu.get_half_carry_flag(),
            "0xFFF + 1 overflows twelve bits: H sets"
        );
        assert!(cpu.get_carry_flag(), "bit-15 carry: C sets");
    }

    // 0x20 JR NZ, e

    #[test]
    fn jr_nz_taken_jumps_backward() {
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
        let (mut cpu, mut bus) = setup(&[0x20, 0x05]); // JR NZ, +5
        cpu.set_zero_flag(false);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 12, "taken JR should cost 12 cycles");
        assert_eq!(cpu.pc, 0xC007, "taken forward jump: 0xC002 + 5 = 0xC007");
    }

    #[test]
    fn jr_nz_not_taken_falls_through() {
        let (mut cpu, mut bus) = setup(&[0x20, 0xFA]); // JR NZ, -6 (ignored)
        cpu.set_zero_flag(true);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 8, "not-taken JR should cost 8 cycles");
        assert_eq!(
            cpu.pc, 0xC002,
            "not taken: pc advances past both bytes, offset not applied"
        );
    }

    // 0x22 LD (HL+), A

    #[test]
    fn ld_hl_inc_a_stores_and_bumps_pointer() {
        let (mut cpu, mut bus) = setup(&[0x22]); // LD (HL+), A
        cpu.set_hl(0xC050);
        cpu.a = 0x5A; // nonzero, so the store is provable against zeroed WRAM

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 8, "LD (HL+), A should take 8 cycles");
        assert_eq!(
            bus.read_byte(0xC050),
            0x5A,
            "A should be stored at the original HL address"
        );
        assert_eq!(cpu.get_hl(), 0xC051, "HL should increment after the store");
    }

    #[test]
    fn ld_hl_inc_a_carries_across_page_boundary() {
        let (mut cpu, mut bus) = setup(&[0x22]); // LD (HL+), A
        cpu.set_hl(0xC0FF);
        cpu.a = 0x77;

        cpu.step(&mut bus);

        assert_eq!(
            bus.read_byte(0xC0FF),
            0x77,
            "store happens at the pre-increment address"
        );
        assert_eq!(cpu.h, 0xC1, "carry must propagate into h");
        assert_eq!(cpu.l, 0x00, "l wraps to zero");
    }

    // 0x46 LD B, (HL)

    #[test]
    fn ld_b_from_hl_reads_memory() {
        let (mut cpu, mut bus) = setup(&[0x46]); // LD B, (HL)
        cpu.set_hl(0xC050);
        bus.write_byte(0xC050, 0x42);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 8, "LD B, (HL) should take 8 cycles");
        assert_eq!(cpu.b, 0x42, "the byte at (HL) lands in B");
    }

    // 0xAF XOR A

    #[test]
    fn xor_a_zeroes_a_and_clears_nhc() {
        let (mut cpu, mut bus) = setup(&[0xAF]); // XOR A
        cpu.a = 0x5A;
        cpu.set_half_carry_flag(true); // pre-set H: proves XOR clears it (AND sets it)

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 4, "XOR A should take 4 cycles");
        assert_eq!(cpu.a, 0x00, "A ^ A is always 0");
        assert!(cpu.get_zero_flag(), "zero result should set Z");
        assert!(!cpu.get_subtract_flag(), "XOR always clears N");
        assert!(!cpu.get_half_carry_flag(), "XOR always clears H");
        assert!(!cpu.get_carry_flag(), "XOR always clears C");
    }

    // 0xB1 OR C

    #[test]
    fn or_c_nonzero_result_clears_z_and_nhc() {
        let (mut cpu, mut bus) = setup(&[0xB1]);
        cpu.a = 0x59;
        cpu.c = 0x5A;
        cpu.set_half_carry_flag(true); // pre-set H: proves OR clears it (AND sets it)

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 4, "OR C should take 4 cycles");
        assert_eq!(cpu.a, 0x5B, "result of A | C lands in A");
        assert!(!cpu.get_zero_flag(), "nonzero result should clear Z");
        assert!(!cpu.get_subtract_flag(), "OR always clears N");
        assert!(!cpu.get_half_carry_flag(), "OR always clears H");
        assert!(!cpu.get_carry_flag(), "OR always clears carry");
    }

    // 0xC9 RET (with 0xCD CALL)

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
    fn call_then_ret_round_trips() {
        let (mut cpu, mut bus) = setup(&[0xCD, 0x00, 0xC1]); // CALL 0xC100
        bus.write_byte(0xC100, 0xC9); // plant RET at the call target

        cpu.step(&mut bus); // execute CALL
        assert_eq!(cpu.pc, 0xC100, "CALL should land on the RET");

        let cycles = cpu.step(&mut bus); // execute RET

        assert_eq!(cycles, 16, "RET should take 16 cycles");
        assert_eq!(
            cpu.pc, 0xC003,
            "RET should return to the instruction after the CALL"
        );
        assert_eq!(
            cpu.sp, 0xFFFE,
            "stack should fully unwind: push then pop nets zero"
        );
    }

    // 0xE6 AND n

    #[test]
    fn and_n_nonzero_result() {
        let (mut cpu, mut bus) = setup(&[0xE6, 0x3C]);
        cpu.a = 0xF0;
        cpu.set_carry_flag(true); // pre-set C: proves AND clears it

        let cycles = cpu.step(&mut bus);

        assert_eq!(cpu.a, 0x30);
        assert_eq!(cycles, 8);
        assert!(!cpu.get_zero_flag());
        assert!(!cpu.get_subtract_flag());
        assert!(cpu.get_half_carry_flag());
        assert!(!cpu.get_carry_flag());
    }

    #[test]
    fn and_n_zero_result() {
        let (mut cpu, mut bus) = setup(&[0xE6, 0x0F]);
        cpu.a = 0xF0;

        let cycles = cpu.step(&mut bus);

        assert_eq!(cpu.a, 0x00);
        assert_eq!(cycles, 8);
        assert!(cpu.get_zero_flag());
        assert!(!cpu.get_subtract_flag());
        assert!(cpu.get_half_carry_flag());
        assert!(!cpu.get_carry_flag());
    }

    // 0xFE CP n

    #[test]
    fn cp_equal_sets_zero_clears_carry() {
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

    // Collapse probes

    #[test]
    fn inc_a_through_collapsed_row() {
        let (mut cpu, mut bus) = setup(&[0x3C]); // INC A
        cpu.a = 0x0F;

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 4, "INC A (register target) should take 4 cycles");
        assert_eq!(cpu.a, 0x10, "0x0F + 1 = 0x10");
        assert!(cpu.get_half_carry_flag(), "low nibble 0xF overflow: H sets");
    }

    #[test]
    fn inc_hl_mem_costs_12() {
        let (mut cpu, mut bus) = setup(&[0x34]); // INC (HL)
        cpu.set_hl(0xC050);
        bus.write_byte(0xC050, 0xFF);

        let cycles = cpu.step(&mut bus);

        assert_eq!(
            cycles, 12,
            "INC (HL) is read-modify-write to memory: 12 cycles"
        );
        assert_eq!(
            bus.read_byte(0xC050),
            0x00,
            "0xFF + 1 wraps to 0x00 in memory"
        );
        assert!(cpu.get_zero_flag(), "wrapped result is zero: Z set");
    }

    #[test]
    fn cp_c_through_collapsed_row() {
        let (mut cpu, mut bus) = setup(&[0xB9]); // CP C
        cpu.a = 0x10;
        cpu.c = 0x01;

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 4, "CP C (register source) should take 4 cycles");
        assert_eq!(cpu.a, 0x10, "CP must not modify A");
        assert!(
            cpu.get_half_carry_flag(),
            "0x10 - 0x01: low-nibble borrow, H sets"
        );
        assert!(
            !cpu.get_carry_flag(),
            "0x10 > 0x01 overall: no full borrow, C clear"
        );
    }
}
