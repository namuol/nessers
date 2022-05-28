use crate::{
  bus_device::{BusDevice, BusDeviceRange},
  cart::Cart,
};

pub trait RangedBusDevice: BusDevice + BusDeviceRange {}
impl<T> RangedBusDevice for T where T: BusDevice + BusDeviceRange {}

#[derive(Clone)]
pub struct Mirror {
  pub start: u16,
  total_size: usize,
}

impl Mirror {
  pub fn new(start: u16, total_size: usize) -> Self {
    Mirror { start, total_size }
  }

  pub fn write(
    &mut self,
    master: &mut dyn RangedBusDevice,
    addr: u16,
    data: u8,
    cart: &mut Cart,
  ) -> Option<()> {
    if !self.in_range(addr) {
      return None;
    }
    let master_addr = master.start() + (addr % master.size() as u16);
    master.write(master_addr, data, cart)
  }

  pub fn read(&mut self, master: &mut dyn RangedBusDevice, addr: u16, cart: &Cart) -> Option<u8> {
    if !self.in_range(addr) {
      return None;
    }
    let master_addr = master.start() + (addr % master.size() as u16);
    master.read(master_addr, cart)
  }

  pub fn safe_read(&self, master: &dyn RangedBusDevice, addr: u16, cart: &Cart) -> Option<u8> {
    if !self.in_range(addr) {
      return None;
    }
    let master_addr = master.start() + (addr % master.size() as u16);
    master.safe_read(master_addr, cart)
  }
}

impl BusDeviceRange for Mirror {
  fn start(&self) -> u16 {
    self.start
  }
  fn size(&self) -> usize {
    self.total_size
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::ram::Ram;

  #[test]
  fn ram_mirror() {
    let mut cart = Cart::from_file("src/test_fixtures/nestest.nes").unwrap();
    let mut ram = Ram::new(0x0000, 32 * 1024);
    let mut mirror = Mirror::new(0x0000, 2 * 32 * 1024);
    mirror.write(&mut ram, 0x0000, 42, &mut cart);
    assert_eq!(mirror.read(&mut ram, 0x8000, &cart), Some(42));
    assert_eq!(mirror.read(&mut ram, 0x0000, &cart), Some(42));
    mirror.write(&mut ram, 0x0001, 43, &mut cart);
    assert_eq!(mirror.read(&mut ram, 0x8001, &cart), Some(43));
    assert_eq!(mirror.read(&mut ram, 0x0001, &cart), Some(43));
    mirror.write(&mut ram, 0x7FFF, 44, &mut cart);
    assert_eq!(mirror.read(&mut ram, 0xFFFF, &cart), Some(44));
    assert_eq!(mirror.read(&mut ram, 0x7FFF, &cart), Some(44));
    mirror.write(&mut ram, 0x7FFE, 45, &mut cart);
    assert_eq!(mirror.read(&mut ram, 0xFFFE, &cart), Some(45));
    assert_eq!(mirror.read(&mut ram, 0x7FFE, &cart), Some(45));
    mirror.write(&mut ram, 0x7FFA, 46, &mut cart);
    assert_eq!(mirror.read(&mut ram, 0xFFFA, &cart), Some(46));
    assert_eq!(mirror.read(&mut ram, 0x7FFA, &cart), Some(46));
  }
}
