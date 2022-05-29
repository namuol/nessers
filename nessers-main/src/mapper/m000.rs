#![allow(unused_comparisons)]

use super::{safe_cpu_read, safe_ppu_read, Mapper};

pub struct M000 {
  num_banks: usize,
}

impl M000 {
  pub fn new(num_banks: usize) -> Self {
    M000 { num_banks }
  }
}

impl Mapper for M000 {
  fn safe_cpu_read(&self, addr: u16) -> Option<usize> {
    safe_cpu_read(self.num_banks, addr)
  }

  fn safe_ppu_read(&self, addr: u16) -> Option<usize> {
    safe_ppu_read(addr)
  }
}
