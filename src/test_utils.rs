pub struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    pub fn next_u8(&mut self) -> u8 {
        (self.next() >> 32) as u8
    }

    pub fn next_u16(&mut self) -> u16 {
        (self.next() >> 48) as u16
    }

    pub fn next_u32(&mut self) -> u32 {
        self.next() as u32
    }
}
