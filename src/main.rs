use crate::bus::Bus;

pub mod bus;
pub mod cpu6502;

const RAM_SIZE: usize = 64 * 1024;

pub struct RAM {
    buf: [u8; RAM_SIZE],
}

impl Bus for RAM {
    fn write(&mut self, addr: u16, data: u8) {
        self.buf[addr as usize] = data;
    }
    fn read(&self, addr: u16) -> u8 {
        self.buf[addr as usize]
    }
}

fn main() {}
