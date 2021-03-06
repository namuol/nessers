pub trait Bus<T> {
  fn safe_read(&self, addr: u16) -> u8;
  fn read(&mut self, addr: u16) -> u8;
  fn write(&mut self, addr: u16, data: u8);

  fn safe_read16(&self, addr: u16) -> u16 {
    let lo = self.safe_read(addr) as u16;
    let hi = self.safe_read(addr + 1) as u16;
    (hi << 8) | lo
  }
  fn read16(&mut self, addr: u16) -> u16 {
    let lo = self.read(addr) as u16;
    let hi = self.read(addr + 1) as u16;
    (hi << 8) | lo
  }
  fn write16(&mut self, addr: u16, data: u16) {
    let lo: u8 = (data << 8) as u8;
    let hi: u8 = (data >> 8) as u8;
    self.write(addr, lo);
    self.write(addr + 1, hi);
  }
}
