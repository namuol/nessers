use crate::bus::Bus;

const RAM_SIZE: usize = 64 * 1024;

pub struct Ram {
  buf: [u8; RAM_SIZE],
}

impl Ram {
  pub fn new() -> Ram {
    Ram {
      buf: [0x00; RAM_SIZE],
    }
  }
}

impl Bus for Ram {
  fn write(&mut self, addr: u16, data: u8) {
    self.buf[addr as usize] = data;
  }
  fn read(&self, addr: u16) -> u8 {
    self.buf[addr as usize]
  }
}
