const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 0, 0, 0, 0, 0],
    [0, 1, 1, 1, 1, 0, 0, 0],
    [1, 0, 0, 1, 1, 1, 1, 1],
];

pub struct Pulse {
    channel: u8,
    pub enabled: bool,
    pub length_counter: u8,
    length_halt: bool,

    duty_mode: u8,
    duty_val: u8,

    timer_period: u16,
    timer_val: u16,

    // Envelope
    env_constant: bool,
    env_loop: bool,
    env_start: bool,
    env_period: u8,
    env_val: u8,
    env_volume: u8,

    // Sweep
    sweep_enabled: bool,
    sweep_negate: bool,
    sweep_reload: bool,
    sweep_period: u8,
    sweep_val: u8,
    sweep_shift: u8,
}

impl Pulse {
    pub fn new(channel: u8) -> Self {
        Self {
            channel,
            enabled: false,
            length_counter: 0,
            length_halt: false,
            duty_mode: 0,
            duty_val: 0,
            timer_period: 0,
            timer_val: 0,
            env_constant: false,
            env_loop: false,
            env_start: false,
            env_period: 0,
            env_val: 0,
            env_volume: 0,
            sweep_enabled: false,
            sweep_negate: false,
            sweep_reload: false,
            sweep_period: 0,
            sweep_val: 0,
            sweep_shift: 0,
        }
    }

    pub fn write_ctrl(&mut self, val: u8) {
        self.duty_mode     = (val >> 6) & 0x03;
        self.length_halt   = val & 0x20 != 0;
        self.env_loop      = val & 0x20 != 0;
        self.env_constant  = val & 0x10 != 0;
        self.env_period    = val & 0x0F;
        self.env_volume    = val & 0x0F;
    }

    pub fn write_sweep(&mut self, val: u8) {
        self.sweep_enabled = val & 0x80 != 0;
        self.sweep_period  = (val >> 4) & 0x07;
        self.sweep_negate  = val & 0x08 != 0;
        self.sweep_shift   = val & 0x07;
        self.sweep_reload  = true;
    }

    pub fn write_timer_lo(&mut self, val: u8) {
        self.timer_period = (self.timer_period & 0xFF00) | val as u16;
    }

    pub fn write_timer_hi(&mut self, val: u8, length_table: &[u8; 32]) {
        self.timer_period = (self.timer_period & 0x00FF) | (((val & 0x07) as u16) << 8);
        if self.enabled {
            self.length_counter = length_table[((val >> 3) & 0x1F) as usize];
        }
        self.env_start = true;
        self.duty_val = 0;
    }

    pub fn set_enabled(&mut self, en: bool) {
        self.enabled = en;
        if !en { self.length_counter = 0; }
    }

    pub fn clock_timer(&mut self) {
        if self.timer_val == 0 {
            self.timer_val = self.timer_period;
            self.duty_val = (self.duty_val + 1) % 8;
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

    pub fn clock_length_sweep(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }

        if self.sweep_reload {
            if self.sweep_enabled && self.sweep_val == 0 {
                self.apply_sweep();
            }
            self.sweep_val = self.sweep_period;
            self.sweep_reload = false;
        } else if self.sweep_val > 0 {
            self.sweep_val -= 1;
        } else {
            if self.sweep_enabled {
                self.apply_sweep();
            }
            self.sweep_val = self.sweep_period;
        }
    }

    fn apply_sweep(&mut self) {
        let delta = self.timer_period >> self.sweep_shift;
        if self.sweep_negate {
            self.timer_period = self.timer_period.saturating_sub(delta + self.channel as u16 - 1);
        } else {
            self.timer_period = self.timer_period.saturating_add(delta);
        }
    }

    pub fn output(&self) -> u8 {
        if !self.enabled
            || self.length_counter == 0
            || DUTY_TABLE[self.duty_mode as usize][self.duty_val as usize] == 0
            || self.timer_period < 8
            || self.timer_period > 0x7FF
        {
            return 0;
        }
        if self.env_constant { self.env_period } else { self.env_val }
    }
}
