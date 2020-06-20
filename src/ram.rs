use crate::bus_device::BusDevice;

pub struct Ram {
  pub size: usize,
  pub start: u16,
  buf: Vec<u8>,
}

impl Ram {
  pub fn new(start: u16, size: usize) -> Ram {
    Ram {
      size,
      start,
      buf: vec![0x00; size],
    }
  }
}

impl BusDevice for Ram {
  fn write(&mut self, addr: u16, data: u8) {
    self.buf[addr as usize] = data;
  }
  fn read(&self, addr: u16) -> u8 {
    self.buf[addr as usize]
  }
}
