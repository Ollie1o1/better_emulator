use crate::bus::Bus;

// Status flags
const C: u8 = 1 << 0; // Carry
const Z: u8 = 1 << 1; // Zero
const I: u8 = 1 << 2; // IRQ Disable
const D: u8 = 1 << 3; // Decimal (unused on NES)
const B: u8 = 1 << 4; // Break
const U: u8 = 1 << 5; // Unused (always 1)
const V: u8 = 1 << 6; // Overflow
const N: u8 = 1 << 7; // Negative

pub struct Cpu {
    pub a:  u8,
    pub x:  u8,
    pub y:  u8,
    pub sp: u8,
    pub pc: u16,
    pub p:  u8,

    pub cycles: u64,
    pub nmi_pending: bool,
    pub irq_pending: bool,
}


impl Cpu {
    pub fn new() -> Self {
        Self {
            a: 0, x: 0, y: 0,
            sp: 0xFD,
            pc: 0,
            p: U | I,
            cycles: 0,
            nmi_pending: false,
            irq_pending: false,
        }
    }

    pub fn reset(&mut self, bus: &mut Bus) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0xFD;
        self.p = U | I;
        let lo = bus.cpu_read(0xFFFC) as u16;
        let hi = bus.cpu_read(0xFFFD) as u16;
        self.pc = (hi << 8) | lo;
        self.cycles = 8;
    }

    /// Execute one instruction. Returns CPU cycles consumed.
    pub fn step(&mut self, bus: &mut Bus) -> u8 {
        // Handle interrupts
        if self.nmi_pending {
            self.nmi_pending = false;
            self.handle_nmi(bus);
            return 7;
        }
        if self.irq_pending && self.p & I == 0 {
            self.irq_pending = false;
            self.handle_irq(bus);
            return 7;
        }

        let opcode = self.read_pc(bus);
        self.execute(opcode, bus)
    }

    fn handle_nmi(&mut self, bus: &mut Bus) {
        self.push_word(bus, self.pc);
        self.push(bus, (self.p | U) & !B);
        self.p |= I;
        let lo = bus.cpu_read(0xFFFA) as u16;
        let hi = bus.cpu_read(0xFFFB) as u16;
        self.pc = (hi << 8) | lo;
    }

    fn handle_irq(&mut self, bus: &mut Bus) {
        self.push_word(bus, self.pc);
        self.push(bus, (self.p | U) & !B);
        self.p |= I;
        let lo = bus.cpu_read(0xFFFE) as u16;
        let hi = bus.cpu_read(0xFFFF) as u16;
        self.pc = (hi << 8) | lo;
    }

    // ---- Memory helpers ----

    fn read_pc(&mut self, bus: &mut Bus) -> u8 {
        let val = bus.cpu_read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    fn read16(&mut self, bus: &mut Bus, addr: u16) -> u16 {
        let lo = bus.cpu_read(addr) as u16;
        let hi = bus.cpu_read(addr.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    fn read16_zp(&mut self, bus: &mut Bus, addr: u8) -> u16 {
        let lo = bus.cpu_read(addr as u16) as u16;
        let hi = bus.cpu_read(addr.wrapping_add(1) as u16) as u16;
        (hi << 8) | lo
    }

    fn push(&mut self, bus: &mut Bus, val: u8) {
        bus.cpu_write(0x0100 | self.sp as u16, val);
        self.sp = self.sp.wrapping_sub(1);
    }

    fn pop(&mut self, bus: &mut Bus) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        bus.cpu_read(0x0100 | self.sp as u16)
    }

    fn push_word(&mut self, bus: &mut Bus, val: u16) {
        self.push(bus, (val >> 8) as u8);
        self.push(bus, val as u8);
    }

    fn pop_word(&mut self, bus: &mut Bus) -> u16 {
        let lo = self.pop(bus) as u16;
        let hi = self.pop(bus) as u16;
        (hi << 8) | lo
    }

    // ---- Flag helpers ----

    fn set_flag(&mut self, flag: u8, val: bool) {
        if val { self.p |= flag; } else { self.p &= !flag; }
    }

    fn set_zn(&mut self, val: u8) {
        self.set_flag(Z, val == 0);
        self.set_flag(N, val & 0x80 != 0);
    }

    // ---- Addressing modes ----

    fn addr_imm(&mut self, _bus: &mut Bus) -> (u16, bool) {
        let a = self.pc;
        self.pc = self.pc.wrapping_add(1);
        (a, false)
    }

    fn addr_zpg(&mut self, bus: &mut Bus) -> (u16, bool) {
        let a = self.read_pc(bus) as u16;
        (a, false)
    }

    fn addr_zpgx(&mut self, bus: &mut Bus) -> (u16, bool) {
        let a = (self.read_pc(bus).wrapping_add(self.x)) as u16;
        (a, false)
    }

    fn addr_zpgy(&mut self, bus: &mut Bus) -> (u16, bool) {
        let a = (self.read_pc(bus).wrapping_add(self.y)) as u16;
        (a, false)
    }

    fn addr_abs(&mut self, bus: &mut Bus) -> (u16, bool) {
        let lo = self.read_pc(bus) as u16;
        let hi = self.read_pc(bus) as u16;
        ((hi << 8) | lo, false)
    }

    fn addr_absx(&mut self, bus: &mut Bus) -> (u16, bool) {
        let lo = self.read_pc(bus) as u16;
        let hi = self.read_pc(bus) as u16;
        let base = (hi << 8) | lo;
        let addr = base.wrapping_add(self.x as u16);
        let crossed = (base & 0xFF00) != (addr & 0xFF00);
        (addr, crossed)
    }

    fn addr_absy(&mut self, bus: &mut Bus) -> (u16, bool) {
        let lo = self.read_pc(bus) as u16;
        let hi = self.read_pc(bus) as u16;
        let base = (hi << 8) | lo;
        let addr = base.wrapping_add(self.y as u16);
        let crossed = (base & 0xFF00) != (addr & 0xFF00);
        (addr, crossed)
    }

    fn addr_indx(&mut self, bus: &mut Bus) -> (u16, bool) {
        let zp = self.read_pc(bus).wrapping_add(self.x);
        let addr = self.read16_zp(bus, zp);
        (addr, false)
    }

    fn addr_indy(&mut self, bus: &mut Bus) -> (u16, bool) {
        let zp = self.read_pc(bus);
        let base = self.read16_zp(bus, zp);
        let addr = base.wrapping_add(self.y as u16);
        let crossed = (base & 0xFF00) != (addr & 0xFF00);
        (addr, crossed)
    }

    fn addr_ind(&mut self, bus: &mut Bus) -> (u16, bool) {
        let lo = self.read_pc(bus) as u16;
        let hi = self.read_pc(bus) as u16;
        let ptr = (hi << 8) | lo;
        // 6502 page-crossing bug for JMP indirect
        let addr_lo = bus.cpu_read(ptr) as u16;
        let addr_hi = bus.cpu_read((ptr & 0xFF00) | ((ptr + 1) & 0x00FF)) as u16;
        ((addr_hi << 8) | addr_lo, false)
    }

    fn addr_rel(&mut self, bus: &mut Bus) -> (u16, bool) {
        let offset = self.read_pc(bus) as i8 as i16;
        let addr = (self.pc as i16).wrapping_add(offset) as u16;
        let crossed = (self.pc & 0xFF00) != (addr & 0xFF00);
        (addr, crossed)
    }

    // ---- Branch helper ----

    fn branch(&mut self, bus: &mut Bus, condition: bool) -> u8 {
        let (addr, crossed) = self.addr_rel(bus);
        if condition {
            self.pc = addr;
            if crossed { 2 } else { 1 }
        } else {
            0
        }
    }

    // ---- ADC / SBC shared ----

    fn adc_impl(&mut self, val: u8) {
        let a = self.a as u16;
        let v = val as u16;
        let c = (self.p & C) as u16;
        let result = a + v + c;
        self.set_flag(C, result > 0xFF);
        self.set_flag(V, (!(a ^ v) & (a ^ result) & 0x80) != 0);
        self.a = result as u8;
        self.set_zn(self.a);
    }

    // ---- Main execute dispatch ----

    fn execute(&mut self, op: u8, bus: &mut Bus) -> u8 {
        match op {
            // ---- LDA ----
            0xA9 => { let (a,_)=self.addr_imm(bus); self.a=bus.cpu_read(a); self.set_zn(self.a); 2 }
            0xA5 => { let (a,_)=self.addr_zpg(bus); self.a=bus.cpu_read(a); self.set_zn(self.a); 3 }
            0xB5 => { let (a,_)=self.addr_zpgx(bus); self.a=bus.cpu_read(a); self.set_zn(self.a); 4 }
            0xAD => { let (a,_)=self.addr_abs(bus); self.a=bus.cpu_read(a); self.set_zn(self.a); 4 }
            0xBD => { let (a,c)=self.addr_absx(bus); self.a=bus.cpu_read(a); self.set_zn(self.a); 4+c as u8 }
            0xB9 => { let (a,c)=self.addr_absy(bus); self.a=bus.cpu_read(a); self.set_zn(self.a); 4+c as u8 }
            0xA1 => { let (a,_)=self.addr_indx(bus); self.a=bus.cpu_read(a); self.set_zn(self.a); 6 }
            0xB1 => { let (a,c)=self.addr_indy(bus); self.a=bus.cpu_read(a); self.set_zn(self.a); 5+c as u8 }

            // ---- LDX ----
            0xA2 => { let (a,_)=self.addr_imm(bus); self.x=bus.cpu_read(a); self.set_zn(self.x); 2 }
            0xA6 => { let (a,_)=self.addr_zpg(bus); self.x=bus.cpu_read(a); self.set_zn(self.x); 3 }
            0xB6 => { let (a,_)=self.addr_zpgy(bus); self.x=bus.cpu_read(a); self.set_zn(self.x); 4 }
            0xAE => { let (a,_)=self.addr_abs(bus); self.x=bus.cpu_read(a); self.set_zn(self.x); 4 }
            0xBE => { let (a,c)=self.addr_absy(bus); self.x=bus.cpu_read(a); self.set_zn(self.x); 4+c as u8 }

            // ---- LDY ----
            0xA0 => { let (a,_)=self.addr_imm(bus); self.y=bus.cpu_read(a); self.set_zn(self.y); 2 }
            0xA4 => { let (a,_)=self.addr_zpg(bus); self.y=bus.cpu_read(a); self.set_zn(self.y); 3 }
            0xB4 => { let (a,_)=self.addr_zpgx(bus); self.y=bus.cpu_read(a); self.set_zn(self.y); 4 }
            0xAC => { let (a,_)=self.addr_abs(bus); self.y=bus.cpu_read(a); self.set_zn(self.y); 4 }
            0xBC => { let (a,c)=self.addr_absx(bus); self.y=bus.cpu_read(a); self.set_zn(self.y); 4+c as u8 }

            // ---- STA ----
            0x85 => { let (a,_)=self.addr_zpg(bus); bus.cpu_write(a, self.a); 3 }
            0x95 => { let (a,_)=self.addr_zpgx(bus); bus.cpu_write(a, self.a); 4 }
            0x8D => { let (a,_)=self.addr_abs(bus); bus.cpu_write(a, self.a); 4 }
            0x9D => { let (a,_)=self.addr_absx(bus); bus.cpu_write(a, self.a); 5 }
            0x99 => { let (a,_)=self.addr_absy(bus); bus.cpu_write(a, self.a); 5 }
            0x81 => { let (a,_)=self.addr_indx(bus); bus.cpu_write(a, self.a); 6 }
            0x91 => { let (a,_)=self.addr_indy(bus); bus.cpu_write(a, self.a); 6 }

            // ---- STX ----
            0x86 => { let (a,_)=self.addr_zpg(bus); bus.cpu_write(a, self.x); 3 }
            0x96 => { let (a,_)=self.addr_zpgy(bus); bus.cpu_write(a, self.x); 4 }
            0x8E => { let (a,_)=self.addr_abs(bus); bus.cpu_write(a, self.x); 4 }

            // ---- STY ----
            0x84 => { let (a,_)=self.addr_zpg(bus); bus.cpu_write(a, self.y); 3 }
            0x94 => { let (a,_)=self.addr_zpgx(bus); bus.cpu_write(a, self.y); 4 }
            0x8C => { let (a,_)=self.addr_abs(bus); bus.cpu_write(a, self.y); 4 }

            // ---- Transfers ----
            0xAA => { self.x = self.a; self.set_zn(self.x); 2 }
            0xA8 => { self.y = self.a; self.set_zn(self.y); 2 }
            0xBA => { self.x = self.sp; self.set_zn(self.x); 2 }
            0x8A => { self.a = self.x; self.set_zn(self.a); 2 }
            0x9A => { self.sp = self.x; 2 }
            0x98 => { self.a = self.y; self.set_zn(self.a); 2 }

            // ---- Stack ----
            0x48 => { self.push(bus, self.a); 3 }
            0x68 => { self.a = self.pop(bus); self.set_zn(self.a); 4 }
            0x08 => { self.push(bus, self.p | B | U); 3 }
            0x28 => { self.p = (self.pop(bus) | U) & !B; 4 }

            // ---- ADC ----
            0x69 => { let (a,_)=self.addr_imm(bus); let v=bus.cpu_read(a); self.adc_impl(v); 2 }
            0x65 => { let (a,_)=self.addr_zpg(bus); let v=bus.cpu_read(a); self.adc_impl(v); 3 }
            0x75 => { let (a,_)=self.addr_zpgx(bus); let v=bus.cpu_read(a); self.adc_impl(v); 4 }
            0x6D => { let (a,_)=self.addr_abs(bus); let v=bus.cpu_read(a); self.adc_impl(v); 4 }
            0x7D => { let (a,c)=self.addr_absx(bus); let v=bus.cpu_read(a); self.adc_impl(v); 4+c as u8 }
            0x79 => { let (a,c)=self.addr_absy(bus); let v=bus.cpu_read(a); self.adc_impl(v); 4+c as u8 }
            0x61 => { let (a,_)=self.addr_indx(bus); let v=bus.cpu_read(a); self.adc_impl(v); 6 }
            0x71 => { let (a,c)=self.addr_indy(bus); let v=bus.cpu_read(a); self.adc_impl(v); 5+c as u8 }

            // ---- SBC ----
            0xE9 => { let (a,_)=self.addr_imm(bus); let v=bus.cpu_read(a); self.adc_impl(v^0xFF); 2 }
            0xE5 => { let (a,_)=self.addr_zpg(bus); let v=bus.cpu_read(a); self.adc_impl(v^0xFF); 3 }
            0xF5 => { let (a,_)=self.addr_zpgx(bus); let v=bus.cpu_read(a); self.adc_impl(v^0xFF); 4 }
            0xED => { let (a,_)=self.addr_abs(bus); let v=bus.cpu_read(a); self.adc_impl(v^0xFF); 4 }
            0xFD => { let (a,c)=self.addr_absx(bus); let v=bus.cpu_read(a); self.adc_impl(v^0xFF); 4+c as u8 }
            0xF9 => { let (a,c)=self.addr_absy(bus); let v=bus.cpu_read(a); self.adc_impl(v^0xFF); 4+c as u8 }
            0xE1 => { let (a,_)=self.addr_indx(bus); let v=bus.cpu_read(a); self.adc_impl(v^0xFF); 6 }
            0xF1 => { let (a,c)=self.addr_indy(bus); let v=bus.cpu_read(a); self.adc_impl(v^0xFF); 5+c as u8 }

            // ---- AND ----
            0x29 => { let (a,_)=self.addr_imm(bus); self.a&=bus.cpu_read(a); self.set_zn(self.a); 2 }
            0x25 => { let (a,_)=self.addr_zpg(bus); self.a&=bus.cpu_read(a); self.set_zn(self.a); 3 }
            0x35 => { let (a,_)=self.addr_zpgx(bus); self.a&=bus.cpu_read(a); self.set_zn(self.a); 4 }
            0x2D => { let (a,_)=self.addr_abs(bus); self.a&=bus.cpu_read(a); self.set_zn(self.a); 4 }
            0x3D => { let (a,c)=self.addr_absx(bus); self.a&=bus.cpu_read(a); self.set_zn(self.a); 4+c as u8 }
            0x39 => { let (a,c)=self.addr_absy(bus); self.a&=bus.cpu_read(a); self.set_zn(self.a); 4+c as u8 }
            0x21 => { let (a,_)=self.addr_indx(bus); self.a&=bus.cpu_read(a); self.set_zn(self.a); 6 }
            0x31 => { let (a,c)=self.addr_indy(bus); self.a&=bus.cpu_read(a); self.set_zn(self.a); 5+c as u8 }

            // ---- EOR ----
            0x49 => { let (a,_)=self.addr_imm(bus); self.a^=bus.cpu_read(a); self.set_zn(self.a); 2 }
            0x45 => { let (a,_)=self.addr_zpg(bus); self.a^=bus.cpu_read(a); self.set_zn(self.a); 3 }
            0x55 => { let (a,_)=self.addr_zpgx(bus); self.a^=bus.cpu_read(a); self.set_zn(self.a); 4 }
            0x4D => { let (a,_)=self.addr_abs(bus); self.a^=bus.cpu_read(a); self.set_zn(self.a); 4 }
            0x5D => { let (a,c)=self.addr_absx(bus); self.a^=bus.cpu_read(a); self.set_zn(self.a); 4+c as u8 }
            0x59 => { let (a,c)=self.addr_absy(bus); self.a^=bus.cpu_read(a); self.set_zn(self.a); 4+c as u8 }
            0x41 => { let (a,_)=self.addr_indx(bus); self.a^=bus.cpu_read(a); self.set_zn(self.a); 6 }
            0x51 => { let (a,c)=self.addr_indy(bus); self.a^=bus.cpu_read(a); self.set_zn(self.a); 5+c as u8 }

            // ---- ORA ----
            0x09 => { let (a,_)=self.addr_imm(bus); self.a|=bus.cpu_read(a); self.set_zn(self.a); 2 }
            0x05 => { let (a,_)=self.addr_zpg(bus); self.a|=bus.cpu_read(a); self.set_zn(self.a); 3 }
            0x15 => { let (a,_)=self.addr_zpgx(bus); self.a|=bus.cpu_read(a); self.set_zn(self.a); 4 }
            0x0D => { let (a,_)=self.addr_abs(bus); self.a|=bus.cpu_read(a); self.set_zn(self.a); 4 }
            0x1D => { let (a,c)=self.addr_absx(bus); self.a|=bus.cpu_read(a); self.set_zn(self.a); 4+c as u8 }
            0x19 => { let (a,c)=self.addr_absy(bus); self.a|=bus.cpu_read(a); self.set_zn(self.a); 4+c as u8 }
            0x01 => { let (a,_)=self.addr_indx(bus); self.a|=bus.cpu_read(a); self.set_zn(self.a); 6 }
            0x11 => { let (a,c)=self.addr_indy(bus); self.a|=bus.cpu_read(a); self.set_zn(self.a); 5+c as u8 }

            // ---- CMP ----
            0xC9 => { let (a,_)=self.addr_imm(bus); let v=bus.cpu_read(a); self.cmp(self.a,v); 2 }
            0xC5 => { let (a,_)=self.addr_zpg(bus); let v=bus.cpu_read(a); self.cmp(self.a,v); 3 }
            0xD5 => { let (a,_)=self.addr_zpgx(bus); let v=bus.cpu_read(a); self.cmp(self.a,v); 4 }
            0xCD => { let (a,_)=self.addr_abs(bus); let v=bus.cpu_read(a); self.cmp(self.a,v); 4 }
            0xDD => { let (a,c)=self.addr_absx(bus); let v=bus.cpu_read(a); self.cmp(self.a,v); 4+c as u8 }
            0xD9 => { let (a,c)=self.addr_absy(bus); let v=bus.cpu_read(a); self.cmp(self.a,v); 4+c as u8 }
            0xC1 => { let (a,_)=self.addr_indx(bus); let v=bus.cpu_read(a); self.cmp(self.a,v); 6 }
            0xD1 => { let (a,c)=self.addr_indy(bus); let v=bus.cpu_read(a); self.cmp(self.a,v); 5+c as u8 }

            // ---- CPX ----
            0xE0 => { let (a,_)=self.addr_imm(bus); let v=bus.cpu_read(a); self.cmp(self.x,v); 2 }
            0xE4 => { let (a,_)=self.addr_zpg(bus); let v=bus.cpu_read(a); self.cmp(self.x,v); 3 }
            0xEC => { let (a,_)=self.addr_abs(bus); let v=bus.cpu_read(a); self.cmp(self.x,v); 4 }

            // ---- CPY ----
            0xC0 => { let (a,_)=self.addr_imm(bus); let v=bus.cpu_read(a); self.cmp(self.y,v); 2 }
            0xC4 => { let (a,_)=self.addr_zpg(bus); let v=bus.cpu_read(a); self.cmp(self.y,v); 3 }
            0xCC => { let (a,_)=self.addr_abs(bus); let v=bus.cpu_read(a); self.cmp(self.y,v); 4 }

            // ---- INC ----
            0xE6 => { let (a,_)=self.addr_zpg(bus); let v=bus.cpu_read(a).wrapping_add(1); bus.cpu_write(a,v); self.set_zn(v); 5 }
            0xF6 => { let (a,_)=self.addr_zpgx(bus); let v=bus.cpu_read(a).wrapping_add(1); bus.cpu_write(a,v); self.set_zn(v); 6 }
            0xEE => { let (a,_)=self.addr_abs(bus); let v=bus.cpu_read(a).wrapping_add(1); bus.cpu_write(a,v); self.set_zn(v); 6 }
            0xFE => { let (a,_)=self.addr_absx(bus); let v=bus.cpu_read(a).wrapping_add(1); bus.cpu_write(a,v); self.set_zn(v); 7 }

            // ---- DEC ----
            0xC6 => { let (a,_)=self.addr_zpg(bus); let v=bus.cpu_read(a).wrapping_sub(1); bus.cpu_write(a,v); self.set_zn(v); 5 }
            0xD6 => { let (a,_)=self.addr_zpgx(bus); let v=bus.cpu_read(a).wrapping_sub(1); bus.cpu_write(a,v); self.set_zn(v); 6 }
            0xCE => { let (a,_)=self.addr_abs(bus); let v=bus.cpu_read(a).wrapping_sub(1); bus.cpu_write(a,v); self.set_zn(v); 6 }
            0xDE => { let (a,_)=self.addr_absx(bus); let v=bus.cpu_read(a).wrapping_sub(1); bus.cpu_write(a,v); self.set_zn(v); 7 }

            // ---- INX/INY/DEX/DEY ----
            0xE8 => { self.x=self.x.wrapping_add(1); self.set_zn(self.x); 2 }
            0xC8 => { self.y=self.y.wrapping_add(1); self.set_zn(self.y); 2 }
            0xCA => { self.x=self.x.wrapping_sub(1); self.set_zn(self.x); 2 }
            0x88 => { self.y=self.y.wrapping_sub(1); self.set_zn(self.y); 2 }

            // ---- ASL ----
            0x0A => { self.set_flag(C,self.a&0x80!=0); self.a<<=1; self.set_zn(self.a); 2 }
            0x06 => { let (a,_)=self.addr_zpg(bus); self.asl_mem(bus,a); 5 }
            0x16 => { let (a,_)=self.addr_zpgx(bus); self.asl_mem(bus,a); 6 }
            0x0E => { let (a,_)=self.addr_abs(bus); self.asl_mem(bus,a); 6 }
            0x1E => { let (a,_)=self.addr_absx(bus); self.asl_mem(bus,a); 7 }

            // ---- LSR ----
            0x4A => { self.set_flag(C,self.a&1!=0); self.a>>=1; self.set_zn(self.a); 2 }
            0x46 => { let (a,_)=self.addr_zpg(bus); self.lsr_mem(bus,a); 5 }
            0x56 => { let (a,_)=self.addr_zpgx(bus); self.lsr_mem(bus,a); 6 }
            0x4E => { let (a,_)=self.addr_abs(bus); self.lsr_mem(bus,a); 6 }
            0x5E => { let (a,_)=self.addr_absx(bus); self.lsr_mem(bus,a); 7 }

            // ---- ROL ----
            0x2A => { let c=self.p&C; self.set_flag(C,self.a&0x80!=0); self.a=(self.a<<1)|c; self.set_zn(self.a); 2 }
            0x26 => { let (a,_)=self.addr_zpg(bus); self.rol_mem(bus,a); 5 }
            0x36 => { let (a,_)=self.addr_zpgx(bus); self.rol_mem(bus,a); 6 }
            0x2E => { let (a,_)=self.addr_abs(bus); self.rol_mem(bus,a); 6 }
            0x3E => { let (a,_)=self.addr_absx(bus); self.rol_mem(bus,a); 7 }

            // ---- ROR ----
            0x6A => { let c=(self.p&C)<<7; self.set_flag(C,self.a&1!=0); self.a=(self.a>>1)|c; self.set_zn(self.a); 2 }
            0x66 => { let (a,_)=self.addr_zpg(bus); self.ror_mem(bus,a); 5 }
            0x76 => { let (a,_)=self.addr_zpgx(bus); self.ror_mem(bus,a); 6 }
            0x6E => { let (a,_)=self.addr_abs(bus); self.ror_mem(bus,a); 6 }
            0x7E => { let (a,_)=self.addr_absx(bus); self.ror_mem(bus,a); 7 }

            // ---- BIT ----
            0x24 => { let (a,_)=self.addr_zpg(bus); let v=bus.cpu_read(a); self.set_flag(Z,self.a&v==0); self.p=(self.p&0x3F)|(v&0xC0); 3 }
            0x2C => { let (a,_)=self.addr_abs(bus); let v=bus.cpu_read(a); self.set_flag(Z,self.a&v==0); self.p=(self.p&0x3F)|(v&0xC0); 4 }

            // ---- Branches ----
            0x10 => { 2 + self.branch(bus, self.p & N == 0) } // BPL
            0x30 => { 2 + self.branch(bus, self.p & N != 0) } // BMI
            0x50 => { 2 + self.branch(bus, self.p & V == 0) } // BVC
            0x70 => { 2 + self.branch(bus, self.p & V != 0) } // BVS
            0x90 => { 2 + self.branch(bus, self.p & C == 0) } // BCC
            0xB0 => { 2 + self.branch(bus, self.p & C != 0) } // BCS
            0xD0 => { 2 + self.branch(bus, self.p & Z == 0) } // BNE
            0xF0 => { 2 + self.branch(bus, self.p & Z != 0) } // BEQ

            // ---- Jumps ----
            0x4C => { let (a,_)=self.addr_abs(bus); self.pc=a; 3 }
            0x6C => { let (a,_)=self.addr_ind(bus); self.pc=a; 5 }

            // ---- JSR / RTS / RTI ----
            0x20 => {
                let lo = self.read_pc(bus) as u16;
                self.push_word(bus, self.pc);
                let hi = bus.cpu_read(self.pc) as u16;
                self.pc = (hi << 8) | lo;
                6
            }
            0x60 => { let a = self.pop_word(bus); self.pc = a.wrapping_add(1); 6 }
            0x40 => {
                self.p = self.pop(bus) | U;
                self.p &= !B;
                let a = self.pop_word(bus);
                self.pc = a;
                6
            }

            // ---- BRK ----
            0x00 => {
                self.pc = self.pc.wrapping_add(1);
                self.push_word(bus, self.pc);
                self.push(bus, self.p | B | U);
                self.p |= I;
                let lo = bus.cpu_read(0xFFFE) as u16;
                let hi = bus.cpu_read(0xFFFF) as u16;
                self.pc = (hi << 8) | lo;
                7
            }

            // ---- Flag ops ----
            0x18 => { self.p &= !C; 2 } // CLC
            0x38 => { self.p |=  C; 2 } // SEC
            0x58 => { self.p &= !I; 2 } // CLI
            0x78 => { self.p |=  I; 2 } // SEI
            0xB8 => { self.p &= !V; 2 } // CLV
            0xD8 => { self.p &= !D; 2 } // CLD
            0xF8 => { self.p |=  D; 2 } // SED

            // ---- NOP ----
            0xEA => 2,

            // ---- Unofficial NOPs (common enough to handle) ----
            0x1A|0x3A|0x5A|0x7A|0xDA|0xFA => 2,
            0x04|0x44|0x64 => { self.pc+=1; 3 }
            0x14|0x34|0x54|0x74|0xD4|0xF4 => { self.pc+=1; 4 }
            0x0C => { self.pc+=2; 4 }
            0x1C|0x3C|0x5C|0x7C|0xDC|0xFC => { self.pc+=2; 4 }
            0x80|0x82|0x89|0xC2|0xE2 => { self.pc+=1; 2 }

            // ---- LAX (unofficial) ----
            0xA7 => { let (a,_)=self.addr_zpg(bus); let v=bus.cpu_read(a); self.a=v; self.x=v; self.set_zn(v); 3 }
            0xB7 => { let (a,_)=self.addr_zpgy(bus); let v=bus.cpu_read(a); self.a=v; self.x=v; self.set_zn(v); 4 }
            0xAF => { let (a,_)=self.addr_abs(bus); let v=bus.cpu_read(a); self.a=v; self.x=v; self.set_zn(v); 4 }
            0xBF => { let (a,c)=self.addr_absy(bus); let v=bus.cpu_read(a); self.a=v; self.x=v; self.set_zn(v); 4+c as u8 }
            0xA3 => { let (a,_)=self.addr_indx(bus); let v=bus.cpu_read(a); self.a=v; self.x=v; self.set_zn(v); 6 }
            0xB3 => { let (a,c)=self.addr_indy(bus); let v=bus.cpu_read(a); self.a=v; self.x=v; self.set_zn(v); 5+c as u8 }

            // ---- SAX (unofficial) ----
            0x87 => { let (a,_)=self.addr_zpg(bus); bus.cpu_write(a, self.a&self.x); 3 }
            0x97 => { let (a,_)=self.addr_zpgy(bus); bus.cpu_write(a, self.a&self.x); 4 }
            0x8F => { let (a,_)=self.addr_abs(bus); bus.cpu_write(a, self.a&self.x); 4 }
            0x83 => { let (a,_)=self.addr_indx(bus); bus.cpu_write(a, self.a&self.x); 6 }

            // ---- DCP (unofficial) ----
            0xC7 => { let (a,_)=self.addr_zpg(bus); let v=bus.cpu_read(a).wrapping_sub(1); bus.cpu_write(a,v); self.cmp(self.a,v); 5 }
            0xD7 => { let (a,_)=self.addr_zpgx(bus); let v=bus.cpu_read(a).wrapping_sub(1); bus.cpu_write(a,v); self.cmp(self.a,v); 6 }
            0xCF => { let (a,_)=self.addr_abs(bus); let v=bus.cpu_read(a).wrapping_sub(1); bus.cpu_write(a,v); self.cmp(self.a,v); 6 }
            0xDF => { let (a,_)=self.addr_absx(bus); let v=bus.cpu_read(a).wrapping_sub(1); bus.cpu_write(a,v); self.cmp(self.a,v); 7 }
            0xDB => { let (a,_)=self.addr_absy(bus); let v=bus.cpu_read(a).wrapping_sub(1); bus.cpu_write(a,v); self.cmp(self.a,v); 7 }
            0xC3 => { let (a,_)=self.addr_indx(bus); let v=bus.cpu_read(a).wrapping_sub(1); bus.cpu_write(a,v); self.cmp(self.a,v); 8 }
            0xD3 => { let (a,_)=self.addr_indy(bus); let v=bus.cpu_read(a).wrapping_sub(1); bus.cpu_write(a,v); self.cmp(self.a,v); 8 }

            // ---- ISB / ISC (unofficial) ----
            0xE7 => { let (a,_)=self.addr_zpg(bus); let v=bus.cpu_read(a).wrapping_add(1); bus.cpu_write(a,v); self.adc_impl(v^0xFF); 5 }
            0xF7 => { let (a,_)=self.addr_zpgx(bus); let v=bus.cpu_read(a).wrapping_add(1); bus.cpu_write(a,v); self.adc_impl(v^0xFF); 6 }
            0xEF => { let (a,_)=self.addr_abs(bus); let v=bus.cpu_read(a).wrapping_add(1); bus.cpu_write(a,v); self.adc_impl(v^0xFF); 6 }
            0xFF => { let (a,_)=self.addr_absx(bus); let v=bus.cpu_read(a).wrapping_add(1); bus.cpu_write(a,v); self.adc_impl(v^0xFF); 7 }
            0xFB => { let (a,_)=self.addr_absy(bus); let v=bus.cpu_read(a).wrapping_add(1); bus.cpu_write(a,v); self.adc_impl(v^0xFF); 7 }
            0xE3 => { let (a,_)=self.addr_indx(bus); let v=bus.cpu_read(a).wrapping_add(1); bus.cpu_write(a,v); self.adc_impl(v^0xFF); 8 }
            0xF3 => { let (a,_)=self.addr_indy(bus); let v=bus.cpu_read(a).wrapping_add(1); bus.cpu_write(a,v); self.adc_impl(v^0xFF); 8 }

            // ---- SLO (unofficial) ----
            0x07 => { let (a,_)=self.addr_zpg(bus); self.slo(bus,a); 5 }
            0x17 => { let (a,_)=self.addr_zpgx(bus); self.slo(bus,a); 6 }
            0x0F => { let (a,_)=self.addr_abs(bus); self.slo(bus,a); 6 }
            0x1F => { let (a,_)=self.addr_absx(bus); self.slo(bus,a); 7 }
            0x1B => { let (a,_)=self.addr_absy(bus); self.slo(bus,a); 7 }
            0x03 => { let (a,_)=self.addr_indx(bus); self.slo(bus,a); 8 }
            0x13 => { let (a,_)=self.addr_indy(bus); self.slo(bus,a); 8 }

            // ---- RLA (unofficial) ----
            0x27 => { let (a,_)=self.addr_zpg(bus); self.rla(bus,a); 5 }
            0x37 => { let (a,_)=self.addr_zpgx(bus); self.rla(bus,a); 6 }
            0x2F => { let (a,_)=self.addr_abs(bus); self.rla(bus,a); 6 }
            0x3F => { let (a,_)=self.addr_absx(bus); self.rla(bus,a); 7 }
            0x3B => { let (a,_)=self.addr_absy(bus); self.rla(bus,a); 7 }
            0x23 => { let (a,_)=self.addr_indx(bus); self.rla(bus,a); 8 }
            0x33 => { let (a,_)=self.addr_indy(bus); self.rla(bus,a); 8 }

            // ---- SRE (unofficial) ----
            0x47 => { let (a,_)=self.addr_zpg(bus); self.sre(bus,a); 5 }
            0x57 => { let (a,_)=self.addr_zpgx(bus); self.sre(bus,a); 6 }
            0x4F => { let (a,_)=self.addr_abs(bus); self.sre(bus,a); 6 }
            0x5F => { let (a,_)=self.addr_absx(bus); self.sre(bus,a); 7 }
            0x5B => { let (a,_)=self.addr_absy(bus); self.sre(bus,a); 7 }
            0x43 => { let (a,_)=self.addr_indx(bus); self.sre(bus,a); 8 }
            0x53 => { let (a,_)=self.addr_indy(bus); self.sre(bus,a); 8 }

            // ---- RRA (unofficial) ----
            0x67 => { let (a,_)=self.addr_zpg(bus); self.rra(bus,a); 5 }
            0x77 => { let (a,_)=self.addr_zpgx(bus); self.rra(bus,a); 6 }
            0x6F => { let (a,_)=self.addr_abs(bus); self.rra(bus,a); 6 }
            0x7F => { let (a,_)=self.addr_absx(bus); self.rra(bus,a); 7 }
            0x7B => { let (a,_)=self.addr_absy(bus); self.rra(bus,a); 7 }
            0x63 => { let (a,_)=self.addr_indx(bus); self.rra(bus,a); 8 }
            0x73 => { let (a,_)=self.addr_indy(bus); self.rra(bus,a); 8 }

            // Catch-all NOP for anything else
            _ => {
                log::warn!("Unimplemented opcode: ${:02X} at PC=${:04X}", op, self.pc.wrapping_sub(1));
                2
            }
        }
    }

    // ---- ALU helpers ----

    fn cmp(&mut self, reg: u8, val: u8) {
        let result = reg.wrapping_sub(val);
        self.set_flag(C, reg >= val);
        self.set_zn(result);
    }

    fn asl_mem(&mut self, bus: &mut Bus, addr: u16) {
        let v = bus.cpu_read(addr);
        self.set_flag(C, v & 0x80 != 0);
        let r = v << 1;
        bus.cpu_write(addr, r);
        self.set_zn(r);
    }

    fn lsr_mem(&mut self, bus: &mut Bus, addr: u16) {
        let v = bus.cpu_read(addr);
        self.set_flag(C, v & 1 != 0);
        let r = v >> 1;
        bus.cpu_write(addr, r);
        self.set_zn(r);
    }

    fn rol_mem(&mut self, bus: &mut Bus, addr: u16) {
        let v = bus.cpu_read(addr);
        let c = self.p & C;
        self.set_flag(C, v & 0x80 != 0);
        let r = (v << 1) | c;
        bus.cpu_write(addr, r);
        self.set_zn(r);
    }

    fn ror_mem(&mut self, bus: &mut Bus, addr: u16) {
        let v = bus.cpu_read(addr);
        let c = (self.p & C) << 7;
        self.set_flag(C, v & 1 != 0);
        let r = (v >> 1) | c;
        bus.cpu_write(addr, r);
        self.set_zn(r);
    }

    fn slo(&mut self, bus: &mut Bus, addr: u16) {
        self.asl_mem(bus, addr);
        self.a |= bus.cpu_read(addr);
        self.set_zn(self.a);
    }

    fn rla(&mut self, bus: &mut Bus, addr: u16) {
        self.rol_mem(bus, addr);
        self.a &= bus.cpu_read(addr);
        self.set_zn(self.a);
    }

    fn sre(&mut self, bus: &mut Bus, addr: u16) {
        self.lsr_mem(bus, addr);
        self.a ^= bus.cpu_read(addr);
        self.set_zn(self.a);
    }

    fn rra(&mut self, bus: &mut Bus, addr: u16) {
        self.ror_mem(bus, addr);
        let v = bus.cpu_read(addr);
        self.adc_impl(v);
    }
}

impl Default for Cpu {
    fn default() -> Self { Self::new() }
}
