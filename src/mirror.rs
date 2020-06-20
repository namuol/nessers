use crate::bus_device::BusDevice;

pub struct Mirror<'a> {
  other: Box<&'a mut dyn BusDevice>,
  other_start: u16,
  start: u16,
}

impl BusDevice for Mirror<'_> {
  fn write(&mut self, addr: u16, data: u8) {
    let other_addr = self.other_start + addr - self.start;
    self.other.write(other_addr, data);
  }
  fn read(&self, addr: u16) -> u8 {
    let other_addr = self.other_start + addr - self.start;
    self.other.read(other_addr)
  }
}
