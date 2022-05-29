#![allow(unused_comparisons)]

use super::*;

pub struct M002 {
  num_banks: usize,
  selected_bank: u8,
}

impl M002 {
  pub fn new(num_banks: usize) -> Self {
    M002 {
      num_banks,
      selected_bank: 0,
    }
  }
}

impl Mapper for M002 {
  fn cpu_write(&mut self, addr: u16, data: u8) -> MappedWrite {
    if addr >= 0x8000 && addr <= 0xFFFF {
      // ```
      // 7  bit  0
      // ---- ----
      // xxxx pPPP
      //      ||||
      //      ++++- Select 16 KB PRG ROM bank for CPU $8000-$BFFF
      //           (UNROM uses bits 2-0; UOROM uses bits 3-0)
      // ```
      //
      // Emulator implementations of iNES mapper 2 treat this as a full 8-bit bank
      // select register, without bus conflicts. This allows the mapper to be used
      // for similar boards that are compatible.
      //
      // TODO: To make use of all 8-bits for a 4 MB PRG ROM, an NES 2.0 header
      // must be used (iNES can only effectively go to 2 MB).
      self.selected_bank = data & 0b0000_0111;
    }

    // Return none because we aren't actually writing anything:
    WSkip
  }
  fn safe_cpu_read(&self, addr: u16) -> MappedRead {
    match addr {
      // CPU $8000-$BFFF: 16 KB switchable PRG ROM bank
      0x8000..=0xBFFF => RAddr(((addr as usize) - 0x8000) + (self.selected_bank as usize) * 0x4000),
      // CPU $C000-$FFFF: 16 KB PRG ROM bank, fixed to the last bank
      0xC000..=0xFFFF => RAddr(((addr as usize) - 0xC000) + (self.num_banks - 1) * 0x4000),
      _ => RSkip,
    }
  }

  fn safe_ppu_read(&self, addr: u16) -> MappedRead {
    safe_ppu_read(addr)
  }
}
