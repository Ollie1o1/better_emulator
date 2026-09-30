//! Delta modulation channel: plays 1-bit delta-encoded samples fetched from
//! CPU memory ($C000-$FFFF). The fetch itself is done by the bus (which owns
//! the cartridge) — see `fetch_address` / `provide_sample`.

pub struct Dmc {
    irq_enabled: bool,
    loop_flag: bool,
    pub irq_flag: bool,

    timer_period: u16,
    timer_val: u16,
    output_level: u8,

    // Sample source
    sample_addr: u16,
    sample_len: u16,
    current_addr: u16,
    pub bytes_remaining: u16,
    sample_buffer: Option<u8>,

    // Output unit
    shift_reg: u8,
    bits_remaining: u8,
    silence: bool,
}

impl Dmc {
    pub fn new() -> Self {
        Self {
            irq_enabled: false,
            loop_flag: false,
            irq_flag: false,
            timer_period: DMC_TABLE[0],
            timer_val: DMC_TABLE[0],
            output_level: 0,
            sample_addr: 0xC000,
            sample_len: 1,
            current_addr: 0xC000,
            bytes_remaining: 0,
            sample_buffer: None,
            shift_reg: 0,
            bits_remaining: 8,
            silence: true,
        }
    }

    pub fn write_flags(&mut self, val: u8) {
        self.irq_enabled  = val & 0x80 != 0;
        self.loop_flag    = val & 0x40 != 0;
        self.timer_period = DMC_TABLE[(val & 0x0F) as usize];
        if !self.irq_enabled { self.irq_flag = false; }
    }

    pub fn write_direct(&mut self, val: u8) {
        self.output_level = val & 0x7F;
    }

    pub fn write_addr(&mut self, val: u8) {
        self.sample_addr = 0xC000 | ((val as u16) << 6);
    }

    pub fn write_length(&mut self, val: u8) {
        self.sample_len = ((val as u16) << 4) + 1;
    }

    /// $4015 bit 4. Also acknowledges the DMC IRQ.
    pub fn set_enabled(&mut self, en: bool) {
        self.irq_flag = false;
        if !en {
            self.bytes_remaining = 0;
        } else if self.bytes_remaining == 0 {
            self.restart();
        }
    }

    fn restart(&mut self) {
        self.current_addr = self.sample_addr;
        self.bytes_remaining = self.sample_len;
    }

    /// Address the memory reader wants to fetch this cycle, if its buffer is empty.
    pub fn fetch_address(&self) -> Option<u16> {
        (self.sample_buffer.is_none() && self.bytes_remaining > 0).then_some(self.current_addr)
    }

    /// Deliver the byte fetched from `fetch_address()`.
    pub fn provide_sample(&mut self, byte: u8) {
        self.sample_buffer = Some(byte);
        // Address wraps from $FFFF back to $8000
        self.current_addr = if self.current_addr == 0xFFFF { 0x8000 } else { self.current_addr + 1 };
        self.bytes_remaining -= 1;
        if self.bytes_remaining == 0 {
            if self.loop_flag {
                self.restart();
            } else if self.irq_enabled {
                self.irq_flag = true;
            }
        }
    }

    /// Clocked once per CPU cycle.
    pub fn clock_timer(&mut self) {
        if self.timer_val > 0 {
            self.timer_val -= 1;
            return;
        }
        self.timer_val = self.timer_period - 1;

        if !self.silence {
            if self.shift_reg & 1 != 0 {
                if self.output_level <= 125 { self.output_level += 2; }
            } else if self.output_level >= 2 {
                self.output_level -= 2;
            }
        }
        self.shift_reg >>= 1;

        self.bits_remaining -= 1;
        if self.bits_remaining == 0 {
            self.bits_remaining = 8;
            match self.sample_buffer.take() {
                Some(b) => { self.silence = false; self.shift_reg = b; }
                None => self.silence = true,
            }
        }
    }

    pub fn output(&self) -> u8 {
        self.output_level
    }
}

// Timer periods in CPU cycles (NTSC)
const DMC_TABLE: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
];
