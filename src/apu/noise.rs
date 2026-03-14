const NOISE_PERIOD_TABLE: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

pub struct Noise {
    pub enabled: bool,
    pub length_counter: u8,
    length_halt: bool,

    mode: bool,
    shift: u16,

    timer_period: u16,
    timer_val: u16,

    env_constant: bool,
    env_loop: bool,
    env_start: bool,
    env_period: u8,
    env_val: u8,
    env_volume: u8,
}

impl Noise {
    pub fn new() -> Self {
        Self {
            enabled: false,
            length_counter: 0,
            length_halt: false,
            mode: false,
            shift: 1,
            timer_period: 0,
            timer_val: 0,
            env_constant: false,
            env_loop: false,
            env_start: false,
            env_period: 0,
            env_val: 0,
            env_volume: 0,
        }
    }

    pub fn write_ctrl(&mut self, val: u8) {
        self.length_halt  = val & 0x20 != 0;
        self.env_loop     = val & 0x20 != 0;
        self.env_constant = val & 0x10 != 0;
        self.env_period   = val & 0x0F;
        self.env_volume   = val & 0x0F;
    }

    pub fn write_period(&mut self, val: u8) {
        self.mode = val & 0x80 != 0;
        self.timer_period = NOISE_PERIOD_TABLE[(val & 0x0F) as usize];
    }

    pub fn write_length(&mut self, val: u8, length_table: &[u8; 32]) {
        if self.enabled {
            self.length_counter = length_table[((val >> 3) & 0x1F) as usize];
        }
        self.env_start = true;
    }

    pub fn set_enabled(&mut self, en: bool) {
        self.enabled = en;
        if !en { self.length_counter = 0; }
    }

    pub fn clock_timer(&mut self) {
        if self.timer_val == 0 {
            self.timer_val = self.timer_period;
            let feedback = if self.mode {
                (self.shift & 1) ^ ((self.shift >> 6) & 1)
            } else {
                (self.shift & 1) ^ ((self.shift >> 1) & 1)
            };
            self.shift = (self.shift >> 1) | (feedback << 14);
        } else {
            self.timer_val -= 1;
        }
    }

    pub fn clock_envelope(&mut self) {
        if self.env_start {
            self.env_start = false;
            self.env_val = 15;
            self.env_volume = self.env_period;
        } else if self.env_val > 0 {
            self.env_val -= 1;
        } else {
            if self.env_loop {
                self.env_val = 15;
            }
            self.env_volume = self.env_period;
        }
    }

    pub fn clock_length(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    pub fn output(&self) -> u8 {
        if !self.enabled || self.length_counter == 0 || self.shift & 1 != 0 {
            return 0;
        }
        if self.env_constant { self.env_period } else { self.env_val }
    }
}
