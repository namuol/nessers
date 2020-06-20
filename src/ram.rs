use crate::bus_device::BusDevice;

pub struct Ram {
  pub size: usize,
  buf: Vec<u8>,
}

impl Ram {
  pub fn new(size: usize) -> Ram {
    Ram {
      size,
      buf: vec![0x00; size],
    }
  }
}

impl BusDevice for Ram {
  fn size(&self) -> usize {
    self.size
  }
  fn write(&mut self, addr: u16, data: u8) {
    self.buf[addr as usize] = data;
  }
  fn read(&self, addr: u16) -> u8 {
    self.buf[addr as usize]
  }
}
