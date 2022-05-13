use crate::cart::Cart;

pub trait BusDevice {
  fn read(&mut self, addr: u16, cart: &Cart) -> Option<u8> {
    self.safe_read(addr, cart)
  }
  fn write(&mut self, addr: u16, data: u8, cart: &Cart) -> Option<()>;
  fn safe_read(&self, addr: u16, cart: &Cart) -> Option<u8>;
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
