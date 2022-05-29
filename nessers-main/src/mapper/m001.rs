#![allow(unused_comparisons)]

use crate::cart::Mirroring;

use super::*;

enum PrgFixAddr {
  _8000,
  _C000,
}
use PrgFixAddr::*;

enum PrgMode {
  _16Kx2(PrgFixAddr),
  _32K,
}

enum ChrMode {
  _4Kx2,
  _8K,
}

pub struct M001 {
  num_prg_banks: usize,
  // Mapper 001 has a unique method for loading data into its registers.
  //
  // It loads the register data in serially to `load`, one bit at a time, by
  // right-shifting the last bit from `data` into `load`. `load` is 5 bits long.
  //
  // `load` doesn't correspond to a specific register; it's really more of a
  // buffer which, once the serial sequence is done, gets written to the desired
  // register, of which there are four on this mapper.
  //
  // How can we tell which register we want to save `load` to? Well, let's just
  // say it's ... clever.
  //
  // On the fifth write (setting the final bit of `load`), we look at the
  // _address_ that we wrote to (just on the fifth write!), and we take the
  // _two_ bits (13 and 14) to determine which of the four registers to write
  // `load` to.
  load: u8,

  // Actual registers:
  control: u8,
  chr_bank_0: u8,
  chr_bank_1: u8,
  prg_bank: u8,

  ram: Vec<u8>,
}

impl M001 {
  pub fn new(num_prg_banks: usize) -> Self {
    M001 {
      num_prg_banks,
      // The default load register has bit 7 set to 1, everything else 0. This
      // way, we can tell how many times we've been written to by counting zeros
      // from the least significant bit.
      //
      // For example:
      //
      // - 1000_0000 - Default
      // - -100_0000 - One write
      // - --10_0000 - Two writes
      // - ---1_0000 - Three writes
      // - ----_1000 - Four writes
      // - ----_-100 - Five writes; we're done!
      //
      // The load register is automatically reset back to `1000_0000` once the
      // fifth write happens.
      load: 0b1000_0000,
      control: 0x1C,
      chr_bank_0: 0x00,
      chr_bank_1: 0x00,
      prg_bank: 0x00,
      ram: vec![],
    }
  }

  fn prg_mode(&self) -> PrgMode {
    match (self.control & 0b01100) >> 2 {
      0 | 1 => PrgMode::_32K,
      2 => PrgMode::_16Kx2(_8000),
      3 => PrgMode::_16Kx2(_C000),
      v => panic!("Unexpected prg mode bit value {}", v),
    }
  }

  fn chr_mode(&self) -> ChrMode {
    if (self.control & 0b10000) != 0 {
      ChrMode::_8K
    } else {
      ChrMode::_4Kx2
    }
  }
}

impl Mapper for M001 {
  fn reset(&mut self) {
    self.chr_bank_0 = 0x00;
    self.chr_bank_1 = 0x00;
    self.prg_bank = 0x00;
    self.load = 0b1000_0000;
    self.control = 0x1C;
  }

  fn cpu_write(&mut self, addr: u16, data: u8) -> Option<usize> {
    if addr >= 0x8000 && addr <= 0xFFFF {
      // If bit 7 is set, we are resetting...
      if (data & 0b1000_0000) != 0 {
        // Reset load register and write Control with (Control OR $0C), locking
        // PRG ROM at $C000-$FFFF to the last bank.
        self.load = 0b1000_0000;
        self.control |= 0x0C;
        return None;
      }

      // ...otherwise we are loading into our shift register serially:
      self.load >>= 1;
      self.load |= 0b1000_0000 & (data << 7);

      // Check to see if this is the fifth write; if the 7th bit (always reset
      // to 1) has been shifted over 5 times, it should be in this position:
      //
      // ----_--X-
      if (self.load & 0b0000_0100) == 0 {
        return None;
      }

      // If this *was* our fifth write, then we want to copy the shift register
      // (`load`) into the appropriate internal register based on bits 13 and 14
      // of the address we're writing to right now.
      match addr {
        0x8000..=0x9FFF => self.control = self.load >> 3,
        0xA000..=0xBFFF => self.chr_bank_0 = self.load >> 3,
        0xC000..=0xDFFF => self.chr_bank_1 = self.load >> 3,
        0xE000..=0xFFFF => self.prg_bank = self.load >> 3,
        _ => {}
      }

      // ...and finally, reset the load shift register:
      self.load = 0b1000_0000;
    }

    None
  }

  fn safe_cpu_read(&self, addr: u16) -> MappedRead {
    match addr {
      // In this range, the mapper actually provides the data through its
      // optional RAM bank.
      //
      // TODO: Should we make this configurable based on the cart's settings?
      0x6000..=0x7FFF => Data(self.ram[(addr & 0x1FFF) as usize]),

      // ```
      // 4bit0
      // -----
      // RPPPP
      // |||||
      // |++++- Select 16 KB PRG ROM bank (low bit ignored in 32 KB mode)
      // +----- MMC1B and later: PRG RAM chip enable (0: enabled; 1: disabled; ignored on MMC1A)
      //        MMC1A: Bit 3 bypasses fixed bank logic in 16K mode (0: affected; 1: bypassed)
      // ```
      0x8000.. => match self.prg_mode() {
        PrgMode::_32K => {
          let bank = ((self.prg_bank & 0b01110) >> 1) as usize;
          Addr(((addr as usize) - 0x8000) + bank * 0x8000)
        }
        PrgMode::_16Kx2(fix_at) => match addr {
          0x8000..=0xBFFF => {
            let bank = match fix_at {
              _8000 => 0,
              _C000 => (self.prg_bank & 0b01111) as usize,
            };
            Addr(((addr as usize) - 0x8000) + bank * 0x4000)
          }
          0xC000..=0xFFFF => {
            let bank = match fix_at {
              _8000 => (self.prg_bank & 0b01111) as usize,
              _C000 => self.num_prg_banks - 1,
            };
            Addr(((addr as usize) - 0xC000) + bank * 0x4000)
          }
          _ => Skip,
        },
      },

      _ => Skip,
    }
  }

  fn safe_ppu_read(&self, addr: u16) -> MappedRead {
    match self.chr_mode() {
      ChrMode::_8K => match addr {
        0x0000..=0x1FFF => {
          let bank = ((self.chr_bank_0 & 0b11110) >> 1) as usize;
          Addr(((addr as usize) - 0x0000) + bank * 0x2000)
        }
        _ => Skip,
      },
      ChrMode::_4Kx2 => match addr {
        0x0000..=0x0FFF => {
          let bank = self.chr_bank_0 as usize;
          Addr(((addr as usize) - 0x0000) + bank * 0x1000)
        }
        0x1000..=0x1FFF => {
          let bank = self.chr_bank_1 as usize;
          Addr(((addr as usize) - 0x1000) + bank * 0x1000)
        }
        _ => Skip,
      },
    }
  }

  fn mirroring(&self) -> Option<Mirroring> {
    match self.control & 0b00011 {
      0 => Some(Mirroring::OneScreenLo),
      1 => Some(Mirroring::OneScreenHi),
      2 => Some(Mirroring::Vertical),
      3 => Some(Mirroring::Horizontal),
      _ => None,
    }
  }
}
