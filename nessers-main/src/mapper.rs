#![allow(unused_comparisons)]

use crate::cart::Mirroring;

pub mod m000;
pub mod m001;
pub mod m002;
pub mod m003;
pub mod m004;

pub enum MappedRead {
  Data(u8),
  RAddr(usize),
  RSkip,
}
use MappedRead::*;

pub enum MappedWrite {
  WAddr(usize),
  Wrote,
  WSkip,
}
use MappedWrite::*;

pub trait Mapper {
  fn safe_cpu_read(&self, addr: u16) -> MappedRead;
  fn cpu_read(&mut self, addr: u16) -> MappedRead {
    self.safe_cpu_read(addr)
  }
  fn cpu_write(&mut self, addr: u16, data: u8) -> MappedWrite {
    match self.safe_cpu_read(addr) {
      RAddr(addr) => WAddr(addr),
      _ => WSkip,
    }
  }
  fn safe_ppu_read(&self, addr: u16) -> MappedRead;
  fn ppu_read(&mut self, addr: u16) -> MappedRead {
    self.safe_ppu_read(addr)
  }
  fn ppu_write(&mut self, addr: u16, data: u8) -> MappedWrite {
    match self.safe_ppu_read(addr) {
      RAddr(addr) => WAddr(addr),
      _ => WSkip,
    }
  }
  fn mirroring(&self) -> Option<Mirroring> {
    None
  }

  fn reset(&mut self) {
    // Default does nothing
  }

  /// This method will be called by the emulator to notify the mapper that a
  /// scanline has been completed, allowing it to do handle that however it
  /// chooses.
  ///
  /// Ordinarily a mapper (e.g. 004 aka MMC3) needs to _detect_ when a scanline
  /// is complete by observing the activity on the PPU bus.
  ///
  /// This PPU bus observing trick is pretty complicated to do correctly (and a
  /// testament of the cleverness of the designers of MMC3), so for now we're
  /// cheating with this hack.
  ///
  /// Most mappers do not need to override this method.
  fn scanline_complete(&mut self) {
    // Default does nothing
  }

  fn irq_active(&mut self) -> bool {
    // Default does nothing
    false
  }

  fn irq_clear(&mut self) {
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
  fn safe_cpu_read(&self, _addr: u16) -> MappedRead {
    panic!("Mapper {:03} not implemented", self.0)
  }

  fn safe_ppu_read(&self, _addr: u16) -> MappedRead {
    panic!("Mapper {:03} not implemented", self.0)
  }
}

pub fn safe_cpu_read(num_banks: usize, addr: u16) -> MappedRead {
  if addr >= 0x8000 && addr <= 0xFFFF {
    // - num_banks > 1 => 32k rom => map 0x8000 to 0x0000
    // - else, this is a 16k rom => mirror 0x8000 thru the full addr range
    RAddr((addr & if num_banks > 1 { 0x7FFF } else { 0x3FFF }) as usize)
  } else {
    RSkip
  }
}

pub fn safe_ppu_read(addr: u16) -> MappedRead {
  if addr >= 0x0000 && addr <= 0x1FFF {
    RAddr(addr as usize)
  } else {
    RSkip
  }
}
