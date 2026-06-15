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
            0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x36 | 0x3E => {
                // LD r, n -- immediate into register (or (HL) for dst 6)
                let dst = (opcode >> 3) & 0x07;
                let n = self.fetch_byte(bus);
                self.write_r8(bus, dst, n);
                if dst == 6 { 12 } else { 8 }
            }
            0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
                // INC r
                let dst = (opcode >> 3) & 0x07;
                let value = self.read_r8(bus, dst);
                let result = self.inc_r8(value);
                self.write_r8(bus, dst, result);
                if dst == 6 { 12 } else { 4 }
            }
            0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
                // DEC r
                let dst = (opcode >> 3) & 0x07;
                let value = self.read_r8(bus, dst);
                let result = self.dec_r8(value);
                self.write_r8(bus, dst, result);
                if dst == 6 { 12 } else { 4 }
            }
            0x0B => {
                // DEC BC
                // Decrement the BC register pair -- no flags
                let bc = self.get_bc();
                self.set_bc(bc.wrapping_sub(1));
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
            0x17 => {
                // RLA -- rotate A left through carry. Z always cleared.
                let old_carry = self.get_carry_flag();
                let new_carry = (self.a & 0x80) != 0;
                self.a = (self.a << 1) | (old_carry as u8);
                self.set_zero_flag(false); // accumulator rotates always clear Z
                self.set_subtract_flag(false);
                self.set_half_carry_flag(false);
                self.set_carry_flag(new_carry);
                4
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
            0x1B => {
                // DEC DE -- no flags
                let de = self.get_de();
                self.set_de(de.wrapping_sub(1));
                8
            }
            0x20 | 0x28 | 0x30 | 0x38 => {
                // JR cc, e -- conditional relative jump (cc = NZ/Z/NC/C)
                let condition = match (opcode >> 3) & 0x03 {
                    0 => !self.get_zero_flag(),  // NZ
                    1 => self.get_zero_flag(),   // Z
                    2 => !self.get_carry_flag(), // NC
                    3 => self.get_carry_flag(),  // C
                    _ => unreachable!(),         // 2-bit mask can't exceed 3
                };
                self.jr_conditional(bus, condition)
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
            0x23 => {
                // INC HL -- no flags
                let hl = self.get_hl();
                self.set_hl(hl.wrapping_add(1));
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
            0x76 => panic!("HALT not yet implemented"), // must precede 0x40..=0x7F
            0x40..=0x7F => {
                // LD r, r' -- register-to-register (0x76 HALT carved out above)
                let dst = (opcode >> 3) & 0x07;
                let src = opcode & 0x07;
                let value = self.read_r8(bus, src);
                self.write_r8(bus, dst, value);

                if dst == 6 || src == 6 { 8 } else { 4 }
            }
            0x80..=0x87 => {
                // ADD A, r
                let src = opcode & 0x07;
                let value = self.read_r8(bus, src);
                self.alu_add(value);
                if src == 6 { 8 } else { 4 }
            }
            0xA0..=0xA7 => {
                // AND r
                let src = opcode & 0x07;
                let value = self.read_r8(bus, src);
                self.alu_and(value);
                if src == 6 { 8 } else { 4 }
            }
            0xA8..=0xAF => {
                // XOR r
                let src = opcode & 0x07;
                let value = self.read_r8(bus, src);
                self.alu_xor(value);
                if src == 6 { 8 } else { 4 }
            }
            0xB0..=0xB7 => {
                // OR r
                let src = opcode & 0x07;
                let value = self.read_r8(bus, src);
                self.alu_or(value);
                if src == 6 { 8 } else { 4 }
            }
            0xB8..=0xBF => {
                // CP r
                let src = opcode & 0x07;
                let value = self.read_r8(bus, src);
                self.alu_cp(value);
                if src == 6 { 8 } else { 4 }
            }
            0xC1 | 0xD1 | 0xE1 | 0xF1 => {
                // POP rr
                let index = (opcode >> 4) & 0x03;
                let value = self.pop_word(bus);
                self.write_r16_stack(index, value);
                12
            }
            0xC2 | 0xCA | 0xD2 | 0xDA => {
                // JP cc, nn -- conditional absolute jump (cc = NZ/Z/NC/C)
                let condition = match (opcode >> 3) & 0x03 {
                    0 => !self.get_zero_flag(),
                    1 => self.get_zero_flag(),
                    2 => !self.get_carry_flag(),
                    3 => self.get_carry_flag(),
                    _ => unreachable!(),
                };
                self.jp_conditional(bus, condition)
            }
            0xC3 => {
                // JP nn
                let addr = self.fetch_word(bus);
                self.pc = addr;
                16
            }
            0xC5 | 0xD5 | 0xE5 | 0xF5 => {
                // PUSH rr
                let index = (opcode >> 4) & 0x03;
                let value = self.read_r16_stack(index);
                self.push_word(bus, value);
                16
            }
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                // RST -- single-byte call to fixed vector (ttt * 8)
                let target = ((opcode >> 3) & 0x07) as u16 * 8;
                self.push_word(bus, self.pc);
                self.pc = target;
                16
            }
            0xC9 => {
                // RET
                self.pc = self.pop_word(bus);
                16
            }
            0xCB => self.step_cb(bus, pc), // hand off to the CB table
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
            0xE2 => {
                // LD (C), A - store A at 0xFF00 plus C
                let addr = 0xFF00 | (self.c as u16);
                bus.write_byte(addr, self.a);
                8
            }
            0xE6 => {
                // AND n
                let n = self.fetch_byte(bus);
                self.alu_and(n);
                8
            }
            0xE9 => {
                // JP HL -- jump to the address in HL (no memory read despite "(HL)" notation)
                self.pc = self.get_hl();
                4
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
            0xFA => {
                // LD A, (nn)
                // Fetch a 16-bit immediate address and load
                // the byte from that address into A
                let addr = self.fetch_word(bus);
                self.a = bus.read_byte(addr);
                16
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

    fn step_cb(&mut self, bus: &mut Bus, pc: u16) -> u8 {
        let cb_opcode = self.fetch_byte(bus);
        match cb_opcode {
            0x12 => {
                // RL D -- rotate D left through carry: old C -> bit 0, bit 7 -> new C
                let value = self.read_r8(bus, 2); // D
                let old_carry = self.get_carry_flag();
                let new_carry = (value & 0x80) != 0;
                let result = (value << 1) | (old_carry as u8);
                self.set_zero_flag(result == 0);
                self.set_subtract_flag(false);
                self.set_half_carry_flag(false);
                self.set_carry_flag(new_carry);
                self.write_r8(bus, 2, result);
                8
            }
            0x23 => {
                // SLA E -- shift E left arithmetically
                let value = self.read_r8(bus, 3); // E
                let carry = (value & 0x80) != 0;
                let result = value << 1;
                self.set_zero_flag(result == 0);
                self.set_subtract_flag(false);
                self.set_half_carry_flag(false);
                self.set_carry_flag(carry);
                self.write_r8(bus, 3, result);
                8
            }
            0x87 => {
                // RES 0, A
                self.a &= !0x01;
                8
            }
            _ => panic!("unimplemented CB opcode {:#04x} at {:#06x}", cb_opcode, pc),
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

    fn read_r16_stack(&self, index: u8) -> u16 {
        // Stack pair table: BC DE HL AF (slot 3 is AF, not SP -- PUSH/POP only)
        match index {
            0 => self.get_bc(),
            1 => self.get_de(),
            2 => self.get_hl(),
            3 => self.get_af(),
            _ => panic!("invalid r16 stack index: {}", index),
        }
    }

    fn write_r16_stack(&mut self, index: u8, value: u16) {
        // Stack pair table: BC DE HL AF. Slot 3 routes through set_af, which
        // masks F's low nibble to zero -- a raw f write would corrupt flags
        match index {
            0 => self.set_bc(value),
            1 => self.set_de(value),
            2 => self.set_hl(value),
            3 => self.set_af(value),
            _ => panic!("invalid r16 stack index: {}", index),
        }
    }

    fn jr_conditional(&mut self, bus: &mut Bus, condition: bool) -> u8 {
        let offset = self.fetch_byte(bus) as i8;
        if condition {
            self.pc = self.pc.wrapping_add(offset as u16);
            12
        } else {
            8
        }
    }

    fn jp_conditional(&mut self, bus: &mut Bus, condition: bool) -> u8 {
        let target = self.fetch_word(bus);
        if condition {
            self.pc = target;
            16
        } else {
            12
        }
    }

    // ALU collapse

    fn alu_and(&mut self, value: u8) {
        self.a &= value;
        self.set_zero_flag(self.a == 0);
        self.set_subtract_flag(false);
        self.set_half_carry_flag(true);
        self.set_carry_flag(false);
    }

    fn alu_cp(&mut self, value: u8) {
        let a = self.a;
        self.set_zero_flag(a == value);
        self.set_subtract_flag(true);
        self.set_half_carry_flag((a & 0x0F) < (value & 0x0F));
        self.set_carry_flag(a < value);
    }

    fn alu_or(&mut self, value: u8) {
        self.a |= value;
        self.set_zero_flag(self.a == 0);
        self.set_subtract_flag(false);
        self.set_half_carry_flag(false);
        self.set_carry_flag(false);
    }

    fn alu_xor(&mut self, value: u8) {
        self.a ^= value;
        self.set_zero_flag(self.a == 0);
        self.set_subtract_flag(false);
        self.set_half_carry_flag(false);
        self.set_carry_flag(false);
    }

    fn alu_add(&mut self, value: u8) {
        let a = self.a;
        let (result, carry) = a.overflowing_add(value);
        self.set_zero_flag(result == 0);
        self.set_subtract_flag(false);
        self.set_half_carry_flag((a & 0x0F) + (value & 0x0F) > 0x0F);
        self.set_carry_flag(carry);
        self.a = result;
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

    // 0x17 RLA -- rotate A left through carry; Z ALWAYS cleared

    #[test]
    fn rla_clears_z_even_when_result_is_zero() {
        let (mut cpu, mut bus) = setup(&[0x17]); // RLA
        cpu.a = 0x80;
        cpu.set_carry_flag(false);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 4, "RLA should take 4 cycles (accumulator rotate)");
        assert_eq!(cpu.a, 0x00, "0x80 rotated: top bit out, 0 carry into bit 0");
        assert!(
            !cpu.get_zero_flag(),
            "RLA ALWAYS clears Z -- even on a zero result"
        );
        assert!(
            cpu.get_carry_flag(),
            "old bit 7 (set) becomes the new carry"
        );
        assert!(!cpu.get_subtract_flag(), "RLA clears N");
        assert!(!cpu.get_half_carry_flag(), "RLA clears H");
    }

    #[test]
    fn rla_rotates_old_carry_into_bit0() {
        let (mut cpu, mut bus) = setup(&[0x17]); // RLA
        cpu.a = 0x00;
        cpu.set_carry_flag(true);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 4, "RLA should take 4 cycles");
        assert_eq!(
            cpu.a, 0x01,
            "old carry feeds into bit 0 -- through-carry rotate"
        );
        assert!(!cpu.get_carry_flag(), "bit 7 was clear: new carry clear");
        assert!(!cpu.get_zero_flag(), "RLA clears Z");
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

    // 0x36 LD (HL), n -- the memory-destination member of the collapsed LD r,n row

    #[test]
    fn ld_hl_mem_n_writes_memory_and_costs_12() {
        let (mut cpu, mut bus) = setup(&[0x36, 0x42]); // LD (HL), 0x42
        cpu.set_hl(0xC050);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 12, "LD (HL), n writes memory: 12 cycles, not 8");
        assert_eq!(
            bus.read_byte(0xC050),
            0x42,
            "the immediate byte lands in memory at HL"
        );
    }

    // 0x38 JR C, e

    #[test]
    fn jr_c_taken_when_carry_set() {
        let (mut cpu, mut bus) = setup(&[0x38, 0x05]); // JR C, +5
        cpu.set_carry_flag(true);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 12, "taken JR should cost 12 cycles");
        assert_eq!(cpu.pc, 0xC007, "carry set: jump taken, 0xC002 + 5 = 0xC007");
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

    // 0xC7..=0xFF RST -- single-byte call to a fixed vector (ttt * 8)

    #[test]
    fn rst_00_pushes_return_and_jumps_to_vector() {
        let (mut cpu, mut bus) = setup(&[0xC7]); // RST 00h
        // setup put pc at 0xC000
        // after fetching the 1-byte RST, the return address is 0xC001

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 16, "RST should take 16 cycles");
        assert_eq!(cpu.pc, 0x0000, "RST 00h jumps to vector 0x0000");
        assert_eq!(cpu.sp, 0xFFFC, "RST pushes 2 bytes: sp -= 2");
        assert_eq!(
            bus.read_word(cpu.sp),
            0xC001,
            "pushed return address is the instruction after RST"
        );
    }

    #[test]
    fn rst_38_computes_high_vector() {
        let (mut cpu, mut bus) = setup(&[0xFF]); // RST 38h
        cpu.step(&mut bus);
        assert_eq!(
            cpu.pc, 0x0038,
            "RST 38h jumps to 0x0038 -- proves ttt decode isn't constant"
        );
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

    // 0xCA JP Z, nn -- condition decode through the collapsed conditional-JP arm

    #[test]
    fn jp_z_taken_when_zero_set() {
        // Z set -> absolute jump taken
        let (mut cpu, mut bus) = setup(&[0xCA, 0x34, 0x12]); // JP Z, 0x1234
        cpu.set_zero_flag(true);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 16, "taken JP should cost 16 cycles");
        assert_eq!(cpu.pc, 0x1234, "Z set: jump taken to the operand address");
    }

    #[test]
    fn jp_z_not_taken_consumes_operand() {
        // Z clear -> not taken
        // pc must still advance past all 3 bytes (0xC003)
        let (mut cpu, mut bus) = setup(&[0xCA, 0x34, 0x12]); // JP Z, 0x1234 (ignored)
        cpu.set_zero_flag(false);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 12, "not-taken JP should cost 12 cycles");
        assert_eq!(
            cpu.pc, 0xC003,
            "not taken: pc past opcode + 2 operand bytes, no jump applied"
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

    // CB 0x23 SLA E -- shift left, bit 7 -> carry, 0 -> bit 0

    #[test]
    fn sla_e_shifts_bit7_into_carry() {
        // 0x81 = 1000_0001 -> 0000_0010 = 0x02, old bit 7 (1) lands in carry
        let (mut cpu, mut bus) = setup(&[0xCB, 0x23]); // SLA E
        cpu.e = 0x81;

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 8, "SLA r should take 8 cycles");
        assert_eq!(cpu.e, 0x02, "0x81 << 1 = 0x02, top bit dropped");
        assert!(cpu.get_carry_flag(), "old bit 7 (set) routes into carry");
        assert!(!cpu.get_zero_flag(), "result 0x02 is nonzero: Z clear");
        assert!(!cpu.get_subtract_flag(), "SLA clears N");
        assert!(!cpu.get_half_carry_flag(), "SLA clears H");
    }

    #[test]
    fn sla_e_clears_carry_when_bit7_clear() {
        // 0x40 = 0100_0000 -> 1000_0000 = 0x80, old bit 7 (0) -> carry clear
        let (mut cpu, mut bus) = setup(&[0xCB, 0x23]); // SLA E
        cpu.e = 0x40;

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 8, "SLA r should take 8 cycles");
        assert_eq!(cpu.e, 0x80, "0x40 << 1 = 0x80");
        assert!(!cpu.get_carry_flag(), "old bit 7 (clear) -> carry clear");
        assert!(!cpu.get_zero_flag(), "result 0x80 is nonzero: Z clear");
    }

    // CB 0x12 RL D -- rotate left through carry (distinct from SLA: bit 0 = old carry)

    #[test]
    fn rl_d_feeds_old_carry_into_bit0() {
        // D = 0x00, carry SET. A shift would give 0x00; a rotate-through-carry
        // pulls the old carry into bit 0 -> 0x01
        let (mut cpu, mut bus) = setup(&[0xCB, 0x12]); // RL D
        cpu.d = 0x00;
        cpu.set_carry_flag(true);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 8, "RL r should take 8 cycles");
        assert_eq!(
            cpu.d, 0x01,
            "old carry feeds into bit 0 -- not a plain shift"
        );
        assert!(!cpu.get_carry_flag(), "bit 7 was clear: new carry clear");
        assert!(!cpu.get_zero_flag(), "result 0x01 is nonzero: Z clear");
        assert!(!cpu.get_subtract_flag(), "RL clears N");
        assert!(!cpu.get_half_carry_flag(), "RL clears H");
    }

    #[test]
    fn rl_d_captures_bit7_into_carry_and_zeroes() {
        // D = 0x80, carry CLEAR. bit 7 (1) -> new carry; old carry (0) -> bit 0
        let (mut cpu, mut bus) = setup(&[0xCB, 0x12]); // RL D
        cpu.d = 0x80;
        cpu.set_carry_flag(false);

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 8, "RL r should take 8 cycles");
        assert_eq!(
            cpu.d, 0x00,
            "0x80 << 1 drops the top bit, old carry 0 fills bit 0"
        );
        assert!(
            cpu.get_carry_flag(),
            "old bit 7 (set) becomes the new carry"
        );
        assert!(cpu.get_zero_flag(), "result is zero: Z set");
    }

    // CB/stack: POP AF must mask F's low nibble

    #[test]
    fn pop_af_masks_low_nibble_of_f() {
        // Stack a value whose low nibble is nonzero: 0x123F. F's low 4 bits
        // must come out zero, because the real F register only uses bits 4-7.
        let (mut cpu, mut bus) = setup(&[0xF1]); // POP AF
        cpu.sp = 0xC100;
        bus.write_word(0xC100, 0x123F); // A=0x12, F=0x3F (low nibble 0xF must clear)

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 12, "POP rr should take 12 cycles");
        assert_eq!(cpu.a, 0x12, "high byte pops into A");
        assert_eq!(
            cpu.get_af() & 0x00FF,
            0x30,
            "F low nibble masked off: 0x3F -> 0x30"
        );
        assert_eq!(cpu.sp, 0xC102, "POP unwinds sp by 2");
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

    #[test]
    fn and_hl_through_collapsed_row() {
        let (mut cpu, mut bus) = setup(&[0xA6]); // AND (HL)
        cpu.a = 0xF0;
        cpu.set_hl(0xC050);
        bus.write_byte(0xC050, 0x3C); // operand in memory
        cpu.set_carry_flag(true); // pre-set: AND must clear it

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 8, "AND (HL) reads memory: 8 cycles, not 4");
        assert_eq!(cpu.a, 0x30, "0xF0 & 0x3C = 0x30");
        assert!(!cpu.get_zero_flag(), "nonzero result: Z clear");
        assert!(!cpu.get_subtract_flag(), "AND clears N");
        assert!(cpu.get_half_carry_flag(), "AND always sets H -- the quirk");
        assert!(!cpu.get_carry_flag(), "AND clears C -- was pre-set true");
    }

    // 0x80..=0x87 ADD A, r

    #[test]
    fn add_a_c_plain_no_carries() {
        // A=0x12 + C=0x34 = 0x46. No nibble overflow, no byte overflow.
        let (mut cpu, mut bus) = setup(&[0x81]); // ADD A, C
        cpu.a = 0x12;
        cpu.c = 0x34;

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 4, "ADD A, r (register) should take 4 cycles");
        assert_eq!(cpu.a, 0x46, "0x12 + 0x34 = 0x46");
        assert!(!cpu.get_zero_flag(), "nonzero result: Z clear");
        assert!(!cpu.get_subtract_flag(), "ADD clears N -- unlike SUB/CP");
        assert!(
            !cpu.get_half_carry_flag(),
            "low nibbles 2+4=6, no overflow: H clear"
        );
        assert!(!cpu.get_carry_flag(), "no byte overflow: C clear");
    }

    #[test]
    fn add_a_c_half_carry_without_carry() {
        // A=0x0F + C=0x01 = 0x10. Low nibbles 0xF+0x1 overflow bit 3 -> H set.
        // Total 0x10 is well under 0xFF -> C clear.
        let (mut cpu, mut bus) = setup(&[0x81]); // ADD A, C
        cpu.a = 0x0F;
        cpu.c = 0x01;

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 4, "ADD A, r should take 4 cycles");
        assert_eq!(cpu.a, 0x10, "0x0F + 0x01 = 0x10");
        assert!(!cpu.get_zero_flag(), "nonzero result: Z clear");
        assert!(!cpu.get_subtract_flag(), "ADD clears N");
        assert!(
            cpu.get_half_carry_flag(),
            "low nibbles overflow bit 3: H set"
        );
        assert!(
            !cpu.get_carry_flag(),
            "0x10 << 0xFF: no byte overflow, C clear"
        );
    }

    #[test]
    fn add_a_c_wraps_sets_carry_and_zero() {
        // A=0xFF + C=0x01 = 0x00 (wraps). Both nibble and byte overflow.
        // Result is zero -> Z SET.
        let (mut cpu, mut bus) = setup(&[0x81]); // ADD A, C
        cpu.a = 0xFF;
        cpu.c = 0x01;

        let cycles = cpu.step(&mut bus);

        assert_eq!(cycles, 4, "ADD A, r should take 4 cycles");
        assert_eq!(cpu.a, 0x00, "0xFF + 0x01 wraps to 0x00");
        assert!(
            cpu.get_zero_flag(),
            "wrapped result is zero: Z SET -- 8-bit ADD computes Z"
        );
        assert!(!cpu.get_subtract_flag(), "ADD clears N");
        assert!(
            cpu.get_half_carry_flag(),
            "0xF + 0x1 overflows bit 3: H set"
        );
        assert!(
            cpu.get_carry_flag(),
            "0xFF + 0x01 overflows the byte: C set"
        );
    }
}
