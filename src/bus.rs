pub trait Bus {
  fn write(&mut self, addr: u16, data: u8);
  fn read(&self, addr: u16 /*, read_only: bool*/) -> u8;
  fn read16(&self, addr: u16) -> u16 {
    let lo = self.read(addr) as u16;
    let hi = self.read(addr + 1) as u16;
    (hi << 8) | lo
  }
}
