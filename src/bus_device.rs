pub trait BusDevice {
  fn read(&self, addr: u16 /*, read_only: bool*/) -> Option<u8>;
  fn write(&mut self, addr: u16, data: u8) -> Option<()>;
}

pub trait BusDeviceRange {
  fn start(&self) -> u16;
  fn size(&self) -> usize;
  fn in_range(&self, addr: u16) -> bool {
    let start = self.start();
    let size = self.size() as usize;
    addr >= start && (addr as usize) < (start as usize) + size
  }
}

pub fn in_range(start: u16, size: usize, addr: u16) -> bool {
  addr >= start && (addr as usize) < (start as usize) + size
}
