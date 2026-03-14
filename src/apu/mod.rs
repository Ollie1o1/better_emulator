mod pulse;
mod triangle;
mod noise;
mod dmc;

use pulse::Pulse;
use triangle::Triangle;
use noise::Noise;
use dmc::Dmc;

// Length counter lookup table
const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14,
    12, 16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30,
];

pub struct Apu {
    pub pulse1: Pulse,
    pub pulse2: Pulse,
    pub triangle: Triangle,
    pub noise: Noise,
    pub dmc: Dmc,

    frame_counter: u16,
    frame_mode: bool,   // false=4-step, true=5-step
    irq_inhibit: bool,
    pub frame_irq: bool,

    sample_buffer: Vec<f32>,
    sample_rate: u32,
    cycle: u64,
    sample_timer: f64,
    samples_per_cpu_cycle: f64,
}

impl Apu {
    pub fn new(sample_rate: u32) -> Self {
        let cpu_clock = 1_789_773.0f64;
        Self {
            pulse1: Pulse::new(1),
            pulse2: Pulse::new(2),
            triangle: Triangle::new(),
            noise: Noise::new(),
            dmc: Dmc::new(),
            frame_counter: 0,
            frame_mode: false,
            irq_inhibit: false,
            frame_irq: false,
            sample_buffer: Vec::with_capacity(2048),
            sample_rate,
            cycle: 0,
            sample_timer: 0.0,
            samples_per_cpu_cycle: sample_rate as f64 / cpu_clock,
        }
    }

    pub fn tick(&mut self) {
        self.cycle += 1;

        // Clock frame counter every ~3728.5 CPU cycles (half-frame = 7457 cycles)
        // Simplified: tick frame sequencer every CPU cycle
        self.frame_counter += 1;
        let step_len: u16 = 7457;

        let step = (self.frame_counter / step_len) as u8;
        let steps = if self.frame_mode { 5 } else { 4 };

        if self.frame_counter >= step_len * steps as u16 {
            self.frame_counter = 0;
        }

        // Clock envelopes and triangle linear counter on every quarter-frame
        if self.frame_counter % step_len == 0 {
            self.pulse1.clock_envelope();
            self.pulse2.clock_envelope();
            self.triangle.clock_linear();
            self.noise.clock_envelope();
        }

        // Clock length counters and sweep on every half-frame (steps 2 and 4/5)
        let is_half = (step == 1) || (step == 3 && !self.frame_mode) || (step == 4 && self.frame_mode);
        if is_half && self.frame_counter % step_len == 0 {
            self.pulse1.clock_length_sweep();
            self.pulse2.clock_length_sweep();
            self.triangle.clock_length();
            self.noise.clock_length();
        }

        // IRQ (4-step mode only, step 4)
        if !self.frame_mode && !self.irq_inhibit && step == 3 {
            self.frame_irq = true;
        }

        // Clock timers every CPU cycle (pulse timers every 2 cycles)
        if self.cycle % 2 == 0 {
            self.pulse1.clock_timer();
            self.pulse2.clock_timer();
            self.noise.clock_timer();
        }
        self.triangle.clock_timer();
        self.dmc.clock_timer();

        // Output sample
        self.sample_timer += self.samples_per_cpu_cycle;
        if self.sample_timer >= 1.0 {
            self.sample_timer -= 1.0;
            self.sample_buffer.push(self.mix());
        }
    }

    pub fn cpu_read(&self, addr: u16) -> u8 {
        match addr {
            0x4015 => {
                let mut val = 0u8;
                if self.pulse1.length_counter > 0   { val |= 0x01; }
                if self.pulse2.length_counter > 0   { val |= 0x02; }
                if self.triangle.length_counter > 0 { val |= 0x04; }
                if self.noise.length_counter > 0    { val |= 0x08; }
                if self.dmc.bytes_remaining > 0      { val |= 0x10; }
                if self.frame_irq                    { val |= 0x40; }
                val
            }
            _ => 0,
        }
    }

    pub fn cpu_write(&mut self, addr: u16, val: u8) {
        match addr {
            0x4000 => self.pulse1.write_ctrl(val),
            0x4001 => self.pulse1.write_sweep(val),
            0x4002 => self.pulse1.write_timer_lo(val),
            0x4003 => self.pulse1.write_timer_hi(val, &LENGTH_TABLE),
            0x4004 => self.pulse2.write_ctrl(val),
            0x4005 => self.pulse2.write_sweep(val),
            0x4006 => self.pulse2.write_timer_lo(val),
            0x4007 => self.pulse2.write_timer_hi(val, &LENGTH_TABLE),
            0x4008 => self.triangle.write_linear(val),
            0x400A => self.triangle.write_timer_lo(val),
            0x400B => self.triangle.write_timer_hi(val, &LENGTH_TABLE),
            0x400C => self.noise.write_ctrl(val),
            0x400E => self.noise.write_period(val),
            0x400F => self.noise.write_length(val, &LENGTH_TABLE),
            0x4010 => self.dmc.write_flags(val),
            0x4011 => self.dmc.write_direct(val),
            0x4012 => self.dmc.write_addr(val),
            0x4013 => self.dmc.write_length(val),
            0x4015 => {
                self.pulse1.set_enabled(val & 0x01 != 0);
                self.pulse2.set_enabled(val & 0x02 != 0);
                self.triangle.set_enabled(val & 0x04 != 0);
                self.noise.set_enabled(val & 0x08 != 0);
                self.dmc.set_enabled(val & 0x10 != 0);
                self.frame_irq = false;
            }
            0x4017 => {
                self.frame_mode = val & 0x80 != 0;
                self.irq_inhibit = val & 0x40 != 0;
                if self.irq_inhibit { self.frame_irq = false; }
                self.frame_counter = 0;
                if self.frame_mode {
                    self.pulse1.clock_envelope();
                    self.pulse2.clock_envelope();
                    self.triangle.clock_linear();
                    self.noise.clock_envelope();
                    self.pulse1.clock_length_sweep();
                    self.pulse2.clock_length_sweep();
                    self.triangle.clock_length();
                    self.noise.clock_length();
                }
            }
            _ => {}
        }
    }

    pub fn drain_samples(&mut self, out: &mut Vec<f32>) {
        out.extend_from_slice(&self.sample_buffer);
        self.sample_buffer.clear();
    }

    fn mix(&self) -> f32 {
        let p1 = self.pulse1.output() as f32;
        let p2 = self.pulse2.output() as f32;
        let t  = self.triangle.output() as f32;
        let n  = self.noise.output() as f32;
        let d  = self.dmc.output() as f32;

        // NES non-linear mixing approximation
        let pulse_out = if p1 + p2 > 0.0 {
            95.88 / ((8128.0 / (p1 + p2)) + 100.0)
        } else {
            0.0
        };
        let tnd_out = if t + n + d > 0.0 {
            159.79 / ((1.0 / (t / 8227.0 + n / 12241.0 + d / 22638.0)) + 100.0)
        } else {
            0.0
        };

        (pulse_out + tnd_out) * 2.0 - 1.0
    }
}
