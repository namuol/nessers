use crate::{
  bus_device::{BusDevice, BusDeviceRange},
  cart::Cart,
};

#[derive(Clone)]
pub struct Ram {
  pub start: u16,
  buf: Vec<u8>,
}

impl Ram {
  pub fn new(start: u16, size: usize) -> Ram {
    Ram {
      start,
      buf: vec![0x00; size],
    }
  }
}

impl BusDeviceRange for Ram {
  fn start(&self) -> u16 {
    self.start
  }
  fn size(&self) -> usize {
    self.buf.len()
  }
}

impl BusDevice for Ram {
  fn write(&mut self, addr: u16, data: u8, _cart: &mut Cart) -> Option<()> {
    if !self.in_range(addr) {
      return None;
    }

    self.buf[addr as usize] = data;
    Some(())
  }

  fn safe_read(&self, addr: u16, _cart: &Cart) -> Option<u8> {
    if !self.in_range(addr) {
      return None;
    }

    Some(self.buf[addr as usize])
  }
}
