use std::fs;

use crate::mapper::{m000::M000, m002::M002, m003::M003, Mapper, MXXX};

const HEADER_START: [u8; 4] = [
  0x4E, // N
  0x45, // E
  0x53, // S
  0x1A, // EOF
];

#[allow(dead_code)]
pub struct Cart {
  hw_mirroring: Mirroring,
  has_ram: bool,
  has_trainer: bool,
  pub mapper_code: u8,
  pub mapper: Box<dyn Mapper>,
  prg: Vec<u8>,
  chr: Vec<u8>,
}
#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Mirroring {
  Horizontal,
  Vertical,
  OneScreenLo,
  OneScreenHi,
}

pub const HEADER_SIZE: usize = 16;

pub const FLAG_MIRRORING: u8 = 0b00000001;
pub const FLAG_HAS_RAM: u8 = 0b00000010;
pub const FLAG_HAS_TRAINER: u8 = 0b00000100;

impl Cart {
  pub fn new(data: &Vec<u8>) -> Result<Cart, &'static str> {
    // Bytes 0-3: Should indicate that this is an iNES file:
    if data[0..4] != HEADER_START {
      return Err("Does not appear to be in the iNES format");
    }

    if data.len() < 16 {
      return Err("Too small to contain header");
    }

    let format_version = (data[7] & 0b00001100) >> 2;
    println!("iNES format version: {}", format_version);

    // if format_version != 1 {
    //   return Err("iNES 1.0 format is the only supported format");
    // }

    // Byte 4: Size of PRG ROM in 16KB increments
    let num_prg_banks = data[4] as usize;
    let prg_size = num_prg_banks * 16 * 1024;

    // Byte 5: Size of CHR ROM in 8KB increments
    let num_chr_banks = data[5] as usize;
    let chr_size = num_chr_banks * 8 * 1024;

    let flags_6 = data[6];
    let hw_mirroring = if flags_6 & FLAG_MIRRORING != 0 {
      Mirroring::Vertical
    } else {
      Mirroring::Horizontal
    };

    let has_ram = flags_6 & FLAG_HAS_RAM != 0;
    let has_trainer = flags_6 & FLAG_HAS_TRAINER != 0;
    let mapper_code_lo = flags_6 & 0xF0;
    let mapper_code_hi = data[7] & 0xF0;

    let prg_start = if has_trainer {
      HEADER_SIZE + 512
    } else {
      HEADER_SIZE
    };
    let chr_start = prg_start + prg_size;

    if chr_size > 0 && data.len() < chr_start + chr_size {
      return Err("File is too small to contain ROM data");
    }

    let mapper_code = mapper_code_hi | (mapper_code_lo >> 4);
    let mapper: Box<dyn Mapper> = match mapper_code {
      000 => Box::new(M000::new(num_prg_banks)),
      002 => Box::new(M002::new(num_prg_banks)),
      003 => Box::new(M003::new(num_prg_banks)),
      n => Box::new(MXXX::new(n)),
    };

    Ok(Cart {
      hw_mirroring,
      has_ram,
      has_trainer,
      mapper_code,
      mapper,
      chr: if chr_size > 0 {
        data[chr_start..chr_start + chr_size].to_vec()
      } else {
        vec![0x00; 1024 * 8]
      },
      prg: data[prg_start..prg_start + prg_size].to_vec(),
    })
  }

  pub fn from_file(filename: &str) -> Result<Cart, &'static str> {
    let contents = fs::read(filename).expect(&format!("Failure reading {}", filename));
    Cart::new(&contents)
  }

  pub fn safe_cpu_read(&self, addr: u16) -> Option<u8> {
    let mapped_addr = self.mapper.safe_cpu_read(addr)?;
    Some(self.prg[mapped_addr as usize])
  }
  pub fn cpu_read(&mut self, addr: u16) -> Option<u8> {
    let mapped_addr = self.mapper.cpu_read(addr)?;
    Some(self.prg[mapped_addr as usize])
  }
  pub fn cpu_write(&mut self, addr: u16, data: u8) -> Option<()> {
    let mapped_addr = self.mapper.cpu_write(addr, data)?;
    self.prg[mapped_addr as usize] = data;
    Some(())
  }

  pub fn ppu_read(&mut self, addr: u16) -> Option<u8> {
    let mapped_addr = self.mapper.ppu_read(addr)?;
    Some(self.chr[mapped_addr as usize])
  }
  pub fn ppu_write(&mut self, addr: u16, data: u8) -> Option<()> {
    let mapped_addr = self.mapper.ppu_write(addr, data)?;
    self.chr[mapped_addr as usize] = data;
    Some(())
  }

  pub fn mirroring(&self) -> Mirroring {
    match self.mapper.mirroring() {
      Some(mirroring) => mirroring,
      None => self.hw_mirroring,
    }
  }

  pub fn reset(&mut self) {
    self.mapper.reset();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn header_invalid() {
    match Cart::new(&vec![0x00; 40 * 1024]) {
      Ok(_) => panic!("Expected cart with all zeroes to fail header parsing"),
      Err(msg) => assert_eq!(msg, "Does not appear to be in the iNES format"),
    }
  }

  #[test]
  fn header_valid() {
    let mut data = vec![
      0x4E,                                   // N
      0x45,                                   // E
      0x53,                                   // S
      0x1A,                                   // EOF
      0x01,                                   // 1 * 16K PRG
      0x01,                                   // 1 * 8K CHR
      (0x10 | FLAG_MIRRORING | FLAG_HAS_RAM), // Lower nybble of mapper code + Flags
      (0x10 | 0x01),                          // Upper nybble of mapper code + iNES version
      // Pad up to 16 bytes, which is the minimum for this function not to
      // return an `Err`.
      //
      // These bytes are actually used by the NES 2.0 format, but for now I'm
      // just focusing on the most basic format.
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
      0x00,
    ];
    // Fill PRG with 0x42
    data.resize(16 + 0 + 16 * 1024, 0x42);
    // Fill CHR with 0x43
    data.resize(16 + 0 + 16 * 1024 + 8 * 1024, 0x43);

    match Cart::new(&data) {
      Ok(cart) => {
        assert_eq!(cart.prg, vec![0x42; 16 * 1024]);
        assert_eq!(cart.chr, vec![0x43; 8 * 1024]);
        assert_eq!(cart.hw_mirroring, Mirroring::Vertical);
        assert_eq!(cart.has_ram, true);
        assert_eq!(cart.has_trainer, false);
      }
      Err(msg) => {
        panic!(
          "Should have successfully parsed header, but failed with message:\n\"{}\"",
          msg
        );
      }
    }
  }
}
