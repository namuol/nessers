#![allow(unused_comparisons)]

use crate::cart::Mirroring;

pub mod m000;
pub mod m002;
pub mod m003;

pub trait Mapper {
  fn safe_cpu_read(&self, addr: u16) -> Option<usize>;
  fn cpu_read(&mut self, addr: u16) -> Option<usize> {
    self.safe_cpu_read(addr)
  }
  fn cpu_write(&mut self, addr: u16, data: u8) -> Option<usize> {
    self.safe_cpu_read(addr)
  }
  fn safe_ppu_read(&self, addr: u16) -> Option<usize>;
  fn ppu_read(&mut self, addr: u16) -> Option<usize> {
    self.safe_ppu_read(addr)
  }
  fn ppu_write(&mut self, addr: u16, data: u8) -> Option<usize> {
    self.safe_ppu_read(addr)
  }
  fn mirroring(&self) -> Option<Mirroring> {
    None
  }

  fn reset(&mut self) {
    // Default does nothing
  }
}

/// Unimplemented mapper
pub struct MXXX(u8);
impl MXXX {
  pub fn new(mapper: u8) -> Self {
    panic!("Mapper {:03} not implemented", mapper)
  }
}

impl Mapper for MXXX {
  fn safe_cpu_read(&self, _addr: u16) -> Option<usize> {
    panic!("Mapper {:03} not implemented", self.0)
  }

  fn safe_ppu_read(&self, _addr: u16) -> Option<usize> {
    panic!("Mapper {:03} not implemented", self.0)
  }
}

pub fn safe_cpu_read(num_banks: usize, addr: u16) -> Option<usize> {
  if addr >= 0x8000 && addr <= 0xFFFF {
    // - num_banks > 1 => 32k rom => map 0x8000 to 0x0000
    // - else, this is a 16k rom => mirror 0x8000 thru the full addr range
    Some((addr & if num_banks > 1 { 0x7FFF } else { 0x3FFF }) as usize)
  } else {
    None
  }
}

pub fn safe_ppu_read(addr: u16) -> Option<usize> {
  if addr >= 0x0000 && addr <= 0x1FFF {
    Some(addr as usize)
  } else {
    None
  }
}
