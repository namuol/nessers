use crate::bus_device::{BusDevice, BusDeviceRange};

pub trait RangedBusDevice: BusDevice + BusDeviceRange {}
impl<T> RangedBusDevice for T where T: BusDevice + BusDeviceRange {}

pub struct Mirror {
  pub start: u16,
  master: Box<dyn RangedBusDevice>,
  total_size: usize,
}

impl Mirror {
  pub fn new(start: u16, master: Box<dyn RangedBusDevice>, total_size: usize) -> Self {
    Mirror {
      start,
      master,
      total_size,
    }
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

impl BusDevice for Mirror {
  fn write(&mut self, addr: u16, data: u8) -> Option<()> {
    let master_addr = addr % self.master.size() as u16;

    self.master.write(master_addr, data)
  }
  fn read(&self, addr: u16) -> Option<u8> {
    let master_addr = addr % self.master.size() as u16;
    self.master.read(master_addr)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::ram::Ram;

  #[test]
  fn ram_mirror() {
    let ram = Ram::new(0x0000, 32 * 1024);
    let mut mirror = Mirror::new(0x0000, Box::new(ram), 2 * 32 * 1024);
    mirror.write(0x0000, 42);
    assert_eq!(mirror.read(0x8000), Some(42));
    assert_eq!(mirror.read(0x0000), Some(42));
    mirror.write(0x0001, 43);
    assert_eq!(mirror.read(0x8001), Some(43));
    assert_eq!(mirror.read(0x0001), Some(43));
    mirror.write(0x7FFF, 44);
    assert_eq!(mirror.read(0xFFFF), Some(44));
    assert_eq!(mirror.read(0x7FFF), Some(44));
    mirror.write(0x7FFE, 45);
    assert_eq!(mirror.read(0xFFFE), Some(45));
    assert_eq!(mirror.read(0x7FFE), Some(45));
    mirror.write(0x7FFA, 46);
    assert_eq!(mirror.read(0xFFFA), Some(46));
    assert_eq!(mirror.read(0x7FFA), Some(46));
  }
}
