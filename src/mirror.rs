use crate::bus_device::BusDevice;

pub struct Mirror {
  master: Box<dyn BusDevice>,
  total_size: usize,
}

impl Mirror {
  pub fn new(master: Box<dyn BusDevice>, total_size: usize) -> Self {
    Mirror { master, total_size }
  }
}

impl BusDevice for Mirror {
  fn size(&self) -> usize {
    self.total_size
  }
  fn write(&mut self, addr: u16, data: u8) {
    let master_addr = addr % self.master.size() as u16;
    self.master.write(master_addr, data)
  }
  fn read(&self, addr: u16) -> u8 {
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
    let ram = Ram::new(32 * 1024);
    let mut mirror = Mirror::new(Box::new(ram), 2 * 32 * 1024);
    mirror.write(0x0000, 42);
    assert_eq!(mirror.read(0x0000), 42);
    assert_eq!(mirror.read(0x8000), 42);
    mirror.write(0x0001, 43);
    assert_eq!(mirror.read(0x0001), 43);
    assert_eq!(mirror.read(0x8001), 43);
    mirror.write(0x7FFF, 44);
    assert_eq!(mirror.read(0x7FFF), 44);
    assert_eq!(mirror.read(0xFFFF), 44);
    mirror.write(0x7FFE, 45);
    assert_eq!(mirror.read(0x7FFE), 45);
    assert_eq!(mirror.read(0xFFFE), 45);
    mirror.write(0x7FFA, 46);
    assert_eq!(mirror.read(0x7FFA), 46);
    assert_eq!(mirror.read(0xFFFA), 46);
  }
}
