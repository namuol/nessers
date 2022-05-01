use std::fs;

use crate::bus_device::{BusDevice, BusDeviceRange};

const HEADER_START: [u8; 4] = [
  0x4E, // N
  0x45, // E
  0x53, // S
  0x1A, // EOF
];

#[derive(PartialEq, Debug)]
pub struct Cart {
  prg: Vec<u8>,
  chr: Vec<u8>,
  mirroring: Mirroring,
  has_ram: bool,
  has_trainer: bool,
  mapper_code: u8,
}
#[derive(PartialEq, Debug)]
pub enum Mirroring {
  Horizontal,
  Vertical,
}

const HEADER_SIZE: usize = 16;

const FLAG_MIRRORING: u8 = 0b00000001;
const FLAG_HAS_RAM: u8 = 0b00000010;
const FLAG_HAS_TRAINER: u8 = 0b00000100;

impl Cart {
  pub fn new(data: &Vec<u8>) -> Result<Cart, &'static str> {
    // Bytes 0-3: Should indicate that this is an iNES file:
    if data[0..4] != HEADER_START {
      return Err("Does not appear to be in the iNES format");
    }

    if data.len() < 16 {
      return Err("Too small to contain header");
    }

    // Byte 4: Size of PRG ROM in 16KB increments
    let prg_size = (data[4] as usize) * 16 * 1024;

    // Byte 5: Size of CHR ROM in 8KB increments
    let chr_size = (data[5] as usize) * 8 * 1024;

    let flags_6 = data[6];
    let mirroring = if flags_6 & FLAG_MIRRORING != 0 {
      Mirroring::Horizontal
    } else {
      Mirroring::Vertical
    };

    let has_ram = flags_6 & FLAG_HAS_RAM != 0;
    let has_trainer = flags_6 & FLAG_HAS_TRAINER != 0;
    let mapper_code_lo = flags_6 & 0xF0;
    let mapper_code_hi = data[7] & 0xF0;

    let format_version = (data[7] & 0b00001100) >> 2;

    if format_version == 2 {
      return Err("iNES 2.0 format is not supported yet");
    }

    let prg_start = if has_trainer {
      HEADER_SIZE + 512
    } else {
      HEADER_SIZE
    };
    let chr_start = prg_start + prg_size;

    if data.len() < chr_start + chr_size {
      return Err("File is too small to contain ROM data");
    }

    Ok(Cart {
      prg: data[prg_start..prg_start + prg_size].to_vec(),
      chr: data[chr_start..chr_start + chr_size].to_vec(),
      mirroring,
      has_ram,
      has_trainer,
      mapper_code: (mapper_code_hi | (mapper_code_lo >> 4)),
    })
  }

  pub fn from_file(filename: &str) -> Result<Cart, &'static str> {
    let contents = fs::read(filename).expect(&format!("Failure reading {}", filename));
    Cart::new(&contents)
  }
}

impl BusDeviceRange for Cart {
  fn size(&self) -> usize {
    match self.mapper_code {
      0 => self.prg.len() * 2,
      code => panic!("Unexpected mapper code {}", code),
    }
  }
  fn start(&self) -> u16 {
    match self.mapper_code {
      0 => 0x8000,
      code => panic!("Unexpected mapper code {}", code),
    }
  }
}

impl BusDevice for Cart {
  fn write(&mut self, addr: u16, data: u8) -> Option<()> {
    let start = self.start() as usize;
    let len = self.prg.len();
    match self.mapper_code {
      0 => {
        if !self.in_range(addr) {
          return None;
        }
        self.prg[((addr as usize) - start) % len] = data;
        Some(())
      }
      code => panic!("Unexpected mapper code {}", code),
    }
  }
  fn read(&self, addr: u16) -> Option<u8> {
    let start = self.start() as usize;
    let len = self.prg.len();
    match self.mapper_code {
      0 => {
        if !self.in_range(addr) {
          return None;
        }
        let internal_addr = ((addr as usize) - start) % len;
        Some(self.prg[internal_addr])
      }
      code => panic!("Unexpected mapper code {}", code),
    }
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
      0x4E,                    // N
      0x45,                    // E
      0x53,                    // S
      0x1A,                    // EOF
      0x01,                    // 1 * 16K PRG
      0x02,                    // 2 * 8K CHR
      (0x10 | FLAG_MIRRORING), // Lower nybble of mapper code + Flags
      (0x10 | 0x00),           // Upper nybble of mapper code + iNES version
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
      Ok(header) => {
        assert_eq!(
          header,
          Cart {
            prg: vec![0x42; 16 * 1024],
            chr: vec![0x43; 8 * 1024],
            mirroring: Mirroring::Horizontal,
            has_ram: false,
            has_trainer: false,
            mapper_code: 0x11,
          }
        );
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
