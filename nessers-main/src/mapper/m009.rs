#![allow(unused_comparisons)]

use serde::__private::ser::FlatMapSerializeMap;

use super::*;

#[derive(Copy, Clone)]
enum ChrLatch {
  FD,
  FE,
}

pub struct M009 {
  num_banks: usize,
  prg_bank: u8,
  chr_bank: [u8; 4],
  chr_latch: [ChrLatch; 2],
  ram: [u8; 8 * 1024],
  mirroring: Option<Mirroring>,
}

impl M009 {
  pub fn new(num_banks: usize) -> Self {
    M009 {
      num_banks,
      prg_bank: 0,
      chr_bank: [0x00; 4],
      chr_latch: [ChrLatch::FD; 2],
      ram: [0x00; 8 * 1024],
      mirroring: None,
    }
  }
}

impl Mapper for M009 {
  fn cpu_write(&mut self, addr: u16, data: u8) -> MappedWrite {
    match addr {
      0xA000..=0xAFFF => {
        // PRG ROM bank select ($A000-$AFFF)
        //
        // ```
        // 7  bit  0
        // ---- ----
        // xxxx PPPP
        //      ||||
        //      ++++- Select 8 KB PRG ROM bank for CPU $8000-$9FFF
        // ```
        self.prg_bank = data & 0b0000_1111;
        Wrote
      }
      0xB000..=0xBFFF => {
        // CHR ROM $FD/0000 bank select ($B000-$BFFF)
        //
        // 7  bit  0
        // ---- ----
        // xxxC CCCC
        //    | ||||
        //    +-++++- Select 4 KB CHR ROM bank for PPU $0000-$0FFF
        //            used when latch 0 = $FD
        self.chr_bank[0] = data & 0b0001_1111;
        Wrote
      }

      0xC000..=0xCFFF => {
        // CHR ROM $FE/0000 bank select ($C000-$CFFF)
        //
        // 7  bit  0
        // ---- ----
        // xxxC CCCC
        //    | ||||
        //    +-++++- Select 4 KB CHR ROM bank for PPU $0000-$0FFF
        //           used when latch 0 = $FE
        self.chr_bank[1] = data & 0b0001_1111;
        Wrote
      }
      0xD000..=0xDFFF => {
        // CHR ROM $FD/1000 bank select ($D000-$DFFF)
        //
        // 7  bit  0
        // ---- ----
        // xxxC CCCC
        //    | ||||
        //    +-++++- Select 4 KB CHR ROM bank for PPU $1000-$1FFF
        //           used when latch 1 = $FD
        self.chr_bank[2] = data & 0b0001_1111;
        Wrote
      }
      0xE000..=0xEFFF => {
        // CHR ROM $FE/1000 bank select ($E000-$EFFF)
        //
        // 7  bit  0
        // ---- ----
        // xxxC CCCC
        //    | ||||
        //    +-++++- Select 4 KB CHR ROM bank for PPU $1000-$1FFF
        //           used when latch 1 = $FE
        self.chr_bank[3] = data & 0b0001_1111;
        Wrote
      }
      0xF000..=0xFFFF => {
        // Mirroring ($F000-$FFFF)
        // 7  bit  0
        // ---- ----
        // xxxx xxxM
        //         |
        //         +- Select nametable mirroring (0: vertical; 1: horizontal)
        self.mirroring = if data & 0b0000_0001 == 1 {
          Some(Mirroring::Horizontal)
        } else {
          Some(Mirroring::Vertical)
        };
        Wrote
      }
      _ => WSkip,
    }
  }

  fn safe_cpu_read(&self, addr: u16) -> MappedRead {
    let addr = addr as usize;
    match addr {
      // CPU $6000-$7FFF: 8 KB PRG RAM bank (PlayChoice version only; contains a 6264 and 74139)
      0x6000..=0x7FFF => Data(self.ram[(addr as usize) % self.ram.len()]),
      // CPU $8000-$9FFF: 8 KB switchable PRG ROM bank
      0x8000..=0x9FFF => RAddr((addr - 0x8000) + (self.prg_bank as usize) * 8 * 1024),
      // CPU $A000-$FFFF: Three 8 KB PRG ROM banks, fixed to the last three banks
      0xA000..=0xFFFF => RAddr((addr - 0xA000) + ((self.num_banks * 2) - 3) * 8 * 1024),
      _ => RSkip,
    }
  }

  fn safe_ppu_read(&self, addr: u16) -> MappedRead {
    let addr = addr as usize;
    match addr {
      // PPU $0000-$0FFF: Two 4 KB switchable CHR ROM banks
      0x0000..=0x0FFF => match self.chr_latch[0] {
        ChrLatch::FD => RAddr((addr - 0x0000) + (self.chr_bank[0] as usize) * 4 * 1024),
        ChrLatch::FE => RAddr((addr - 0x0000) + (self.chr_bank[1] as usize) * 4 * 1024),
      },
      // PPU $1000-$1FFF: Two 4 KB switchable CHR ROM banks
      0x1000..=0x1FFF => match self.chr_latch[1] {
        ChrLatch::FD => RAddr((addr - 0x1000) + (self.chr_bank[2] as usize) * 4 * 1024),
        ChrLatch::FE => RAddr((addr - 0x1000) + (self.chr_bank[3] as usize) * 4 * 1024),
      },
      _ => RSkip,
    }
  }

  // The actual mapping occurs in `safe_ppu_read` since we want to reuse that
  // for any addresses that don't match the special latch addresses which
  // auto-switch banks, which we do below since the non-safe method allows us to
  // change our state.
  fn ppu_read(&mut self, addr: u16) -> MappedRead {
    let result = self.safe_ppu_read(addr);

    match addr {
      // PPU reads $0FD8: latch 0 is set to $FD for subsequent reads
      0x0FD8 => {
        self.chr_latch[0] = ChrLatch::FD;
      }
      // PPU reads $0FE8: latch 0 is set to $FE for subsequent reads
      0x0FE8 => {
        self.chr_latch[0] = ChrLatch::FE;
      }
      // PPU reads $1FD8 through $1FDF: latch 1 is set to $FD for subsequent reads
      0x1FD8..=0x1FDF => {
        self.chr_latch[1] = ChrLatch::FD;
      }
      // PPU reads $1FE8 through $1FEF: latch 1 is set to $FE for subsequent reads
      0x1FE8..=0x1FEF => {
        self.chr_latch[1] = ChrLatch::FE;
      }
      _ => {}
    }

    result
  }

  fn mirroring(&self) -> Option<Mirroring> {
    self.mirroring
  }
}
