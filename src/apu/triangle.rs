const TRIANGLE_TABLE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
];

pub struct Triangle {
    pub enabled: bool,
    pub length_counter: u8,
    length_halt: bool,
    control: bool,

    linear_counter: u8,
    linear_period: u8,
    linear_reload: bool,

    timer_period: u16,
    timer_val: u16,
    duty_val: u8,
}

impl Triangle {
    pub fn new() -> Self {
        Self {
            enabled: false,
            length_counter: 0,
            length_halt: false,
            control: false,
            linear_counter: 0,
            linear_period: 0,
            linear_reload: false,
            timer_period: 0,
            timer_val: 0,
            duty_val: 0,
        }
    }

    pub fn write_linear(&mut self, val: u8) {
        self.control = val & 0x80 != 0;
        self.length_halt = val & 0x80 != 0;
        self.linear_period = val & 0x7F;
    }

    pub fn write_timer_lo(&mut self, val: u8) {
        self.timer_period = (self.timer_period & 0xFF00) | val as u16;
    }

    pub fn write_timer_hi(&mut self, val: u8, length_table: &[u8; 32]) {
        self.timer_period = (self.timer_period & 0x00FF) | (((val & 0x07) as u16) << 8);
        if self.enabled {
            self.length_counter = length_table[((val >> 3) & 0x1F) as usize];
        }
        self.linear_reload = true;
    }

    pub fn set_enabled(&mut self, en: bool) {
        self.enabled = en;
        if !en { self.length_counter = 0; }
    }

    pub fn clock_timer(&mut self) {
        if self.timer_val == 0 {
            self.timer_val = self.timer_period;
            if self.length_counter > 0 && self.linear_counter > 0 {
                self.duty_val = (self.duty_val + 1) % 32;
            }
        } else {
            self.timer_val -= 1;
        }
    }

    pub fn clock_linear(&mut self) {
        if self.linear_reload {
            self.linear_counter = self.linear_period;
        } else if self.linear_counter > 0 {
            self.linear_counter -= 1;
        }
        if !self.control {
            self.linear_reload = false;
        }
    }

    pub fn clock_length(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    pub fn output(&self) -> u8 {
        if !self.enabled || self.length_counter == 0 || self.linear_counter == 0 {
            return 0;
        }
        TRIANGLE_TABLE[self.duty_val as usize]
    }
}
