#![allow(unused_comparisons)]

use super::{safe_cpu_read, Mapper};

pub struct M003 {
  num_prg_banks: usize,
  selected_bank: u8,
}

impl M003 {
  pub fn new(num_prg_banks: usize) -> Self {
    M003 {
      num_prg_banks,
      selected_bank: 0,
    }
  }
}

impl Mapper for M003 {
  fn cpu_write(&mut self, addr: u16, data: u8) -> Option<usize> {
    if addr >= 0x8000 && addr <= 0xFFFF {
      // ```
      // 7  bit  0
      // ---- ----
      // cccc ccCC
      // |||| ||||
      // ++++-++++- Select 8 KB CHR ROM bank for PPU $0000-$1FFF
      // ```
      //
      // CNROM only implements the lowest 2 bits, capping it at 32 KiB CHR.
      // Other boards may implement 4 or more bits for larger CHR.
      self.selected_bank = data & 0b0000_0011;
    }

    // Return none because we aren't actually writing anything:
    None
  }
  fn safe_cpu_read(&self, addr: u16) -> Option<usize> {
    safe_cpu_read(self.num_prg_banks, addr)
  }

  fn safe_ppu_read(&self, addr: u16) -> Option<usize> {
    match addr {
      0x0000..=0x1FFF => Some((addr as usize) + (self.selected_bank as usize) * 0x2000),
      _ => None,
    }
  }
}
