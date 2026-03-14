pub mod buttons {
    pub const A:      u8 = 1 << 0;
    pub const B:      u8 = 1 << 1;
    pub const SELECT: u8 = 1 << 2;
    pub const START:  u8 = 1 << 3;
    pub const UP:     u8 = 1 << 4;
    pub const DOWN:   u8 = 1 << 5;
    pub const LEFT:   u8 = 1 << 6;
    pub const RIGHT:  u8 = 1 << 7;
}

pub struct Controller {
    pub buttons: u8,
    strobe: bool,
    shift: u8,
}

impl Controller {
    pub fn new() -> Self {
        Self { buttons: 0, strobe: false, shift: 0 }
    }

    pub fn write(&mut self, val: u8) {
        self.strobe = val & 1 == 1;
        if self.strobe {
            self.shift = self.buttons;
        }
    }

    pub fn read(&mut self) -> u8 {
        if self.strobe {
            return ((self.buttons & buttons::A) != 0) as u8 | 0x40;
        }
        let bit = self.shift & 1;
        self.shift >>= 1;
        self.shift |= 0x80;
        bit | 0x40
    }
}

impl Default for Controller {
    fn default() -> Self { Self::new() }
}
