pub struct Dmc {
    pub enabled: bool,
    irq_enabled: bool,
    loop_flag: bool,
    timer_period: u16,
    timer_val: u16,
    output_level: u8,
    sample_addr: u16,
    sample_len: u16,
    pub bytes_remaining: u16,
    shift_reg: u8,
    bits_remaining: u8,
    silence: bool,
}

impl Dmc {
    pub fn new() -> Self {
        Self {
            enabled: false,
            irq_enabled: false,
            loop_flag: false,
            timer_period: 0,
            timer_val: 0,
            output_level: 0,
            sample_addr: 0,
            sample_len: 0,
            bytes_remaining: 0,
            shift_reg: 0,
            bits_remaining: 0,
            silence: true,
        }
    }

    pub fn write_flags(&mut self, val: u8) {
        self.irq_enabled  = val & 0x80 != 0;
        self.loop_flag    = val & 0x40 != 0;
        self.timer_period = DMC_TABLE[(val & 0x0F) as usize];
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

    pub fn set_enabled(&mut self, en: bool) {
        self.enabled = en;
        if !en {
            self.bytes_remaining = 0;
        } else if self.bytes_remaining == 0 {
            self.bytes_remaining = self.sample_len;
        }
    }

    pub fn clock_timer(&mut self) {
        if self.timer_val == 0 {
            self.timer_val = self.timer_period;
            if !self.silence {
                if self.shift_reg & 1 != 0 {
                    if self.output_level <= 125 { self.output_level += 2; }
                } else {
                    if self.output_level >= 2   { self.output_level -= 2; }
                }
                self.shift_reg >>= 1;
            }
            if self.bits_remaining == 0 {
                self.bits_remaining = 8;
                self.silence = self.bytes_remaining == 0;
            }
            if self.bits_remaining > 0 {
                self.bits_remaining -= 1;
            }
        } else {
            self.timer_val -= 1;
        }
    }

    pub fn output(&self) -> u8 {
        self.output_level
    }
}

const DMC_TABLE: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
];
