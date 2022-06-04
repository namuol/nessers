#![allow(unused_comparisons)]

use super::*;

pub struct M004 {
  num_prg_banks: usize,

  selected_register: Option<u8>,
  registers: [u8; 8],
  ram: [u8; 8 * 1024],
  prg_bank_mode: PrgBankMode,
  chr_bank_mode: ChrBankMode,

  mirroring: Option<Mirroring>,

  irq_reload: u8,
  irq_counter: u8,
  irq_enabled: bool,
  irq_active: bool,
}

enum PrgBankMode {
  _8000_Swap_C000_Fixed,
  _C000_Swap_8000_Fixed,
}
use PrgBankMode::*;
enum ChrBankMode {
  _2x2K_4x1K,
  _4x1K_2x2K,
}
use ChrBankMode::*;

impl M004 {
  pub fn new(num_prg_banks: usize) -> Self {
    M004 {
      // We have 8k-byte bank sizes but our cart implementation assumes 16k-byte
      // bank sizes, so we multiply the bank count provided by the cart by 2
      // here:
      num_prg_banks: num_prg_banks * 2,
      selected_register: None,
      registers: [0b0000_0000; 8],
      ram: [0x00; 8 * 1024],

      prg_bank_mode: PrgBankMode::_C000_Swap_8000_Fixed,
      chr_bank_mode: ChrBankMode::_2x2K_4x1K,

      mirroring: None,

      irq_reload: 0x00,
      irq_counter: 0x00,
      irq_enabled: false,
      irq_active: false,
    }
  }

  fn prg_bank(&self, num: usize) -> usize {
    match self.prg_bank_mode {
      _8000_Swap_C000_Fixed => match num {
        // $8000-$9FFF
        0 => ((self.registers[6] & 0b0011_1111) as usize) * (8 * 1024),
        // $A000-$BFFF
        1 => ((self.registers[7] & 0b0011_1111) as usize) * (8 * 1024),
        // $C000-$DFFF is fixed to the second-to-last bank
        2 => (self.num_prg_banks - 2) * (8 * 1024),
        // $E000-$FFFF always maps to the last bank
        3 => (self.num_prg_banks - 1) * (8 * 1024),
        _ => panic!("Expected bank 0-3 but got {}", num),
      },
      _C000_Swap_8000_Fixed => match num {
        // $8000-$9FFF is fixed to the second-to-last bank
        0 => (self.num_prg_banks - 2) * (8 * 1024),
        // $A000-$BFFF
        1 => ((self.registers[7] & 0b0011_1111) as usize) * (8 * 1024),
        // $C000-$DFFF
        2 => ((self.registers[6] & 0b0011_1111) as usize) * (8 * 1024),
        // $E000-$FFFF always maps to the last bank
        3 => (self.num_prg_banks - 1) * (8 * 1024),
        _ => panic!("Expected bank 0-3 but got {}", num),
      },
    }
  }

  fn chr_bank(&self, num: usize) -> usize {
    match self.chr_bank_mode {
      _2x2K_4x1K => match num {
        0 => (((self.registers[0] & 0b1111_1110) as usize) + 0) * 1024,
        1 => (((self.registers[0] & 0b1111_1110) as usize) + 1) * 1024,

        2 => (((self.registers[1] & 0b1111_1110) as usize) + 0) * 1024,
        3 => (((self.registers[1] & 0b1111_1110) as usize) + 1) * 1024,

        4 => (self.registers[2] as usize) * 1024,
        5 => (self.registers[3] as usize) * 1024,
        6 => (self.registers[4] as usize) * 1024,
        7 => (self.registers[5] as usize) * 1024,
        _ => panic!("Expected bank 0-7 but got {}", num),
      },
      _4x1K_2x2K => match num {
        1 => (self.registers[2] as usize) * 1024,
        2 => (self.registers[3] as usize) * 1024,
        3 => (self.registers[4] as usize) * 1024,
        4 => (self.registers[5] as usize) * 1024,

        5 => (((self.registers[0] & 0b1111_1110) as usize) + 0) * 1024,
        6 => (((self.registers[0] & 0b1111_1110) as usize) + 1) * 1024,

        7 => (((self.registers[1] & 0b1111_1110) as usize) + 0) * 1024,
        8 => (((self.registers[1] & 0b1111_1110) as usize) + 1) * 1024,
        _ => panic!("Expected bank 0-7 but got {}", num),
      },
    }
  }
}

impl Mapper for M004 {
  fn reset(&mut self) {
    self.selected_register = None;
    self.registers = [0b0000_0000; 8];

    self.prg_bank_mode = _8000_Swap_C000_Fixed;
    self.registers[6] = 0;
    self.registers[7] = 1;

    self.chr_bank_mode = _2x2K_4x1K;

    self.mirroring = None;

    self.irq_enabled = false;
    self.irq_active = false;
    self.irq_counter = 0x0000;
    self.irq_reload = 0x0000;
  }

  fn cpu_write(&mut self, addr: u16, data: u8) -> MappedWrite {
    match (addr, (addr % 2) != 0) {
      (0x6000..=0x7FFF, _) => {
        self.ram[(addr - 0x6000) as usize] = data;
        Wrote
      }

      // Bank select ($8000-$9FFE, even)
      (0x8000..=0x9FFE, false) => {
        // Select bank register to write to on next odd write:
        self.selected_register = Some(0b0000_0111 & data);

        self.prg_bank_mode = if (0b0100_0000 & data) == 0 {
          _8000_Swap_C000_Fixed
        } else {
          _C000_Swap_8000_Fixed
        };
        self.chr_bank_mode = if (0b1000_0000 & data) == 0 {
          _2x2K_4x1K
        } else {
          _4x1K_2x2K
        };

        Wrote
      }
      // Bank data ($8001-$9FFF, odd)
      (0x8001..=0x9FFF, true) => match self.selected_register {
        // Ignore when our register is somehow out of range:
        Some(reg @ 0..=7) => {
          self.registers[reg as usize] = data;
          Wrote
        }
        _ => WSkip,
      },
      // Mirroring ($A000-$BFFE, even)
      (0xA000..=0xBFFE, false) => {
        self.mirroring = if (0b0000_0001 & data) == 0 {
          Some(Mirroring::Vertical)
        } else {
          Some(Mirroring::Horizontal)
        };

        Wrote
      }
      // PRG RAM protect ($A001-$BFFF, odd)
      (0xA001..=0xBFFF, true) => {
        // Disabling PRG RAM through bit 7 causes reads from the PRG RAM region
        // to return open bus.
        //
        // Though these bits are functional on the MMC3, their main purpose is
        // to write-protect save RAM during power-off. Many emulators choose not
        // to implement them as part of iNES Mapper 4 to avoid an
        // incompatibility with the MMC6.
        Wrote
      }
      // IRQ latch ($C000-$DFFE, even)
      (0xC000..=0xDFFE, false) => {
        // This register specifies the IRQ counter reload value. When the IRQ
        // counter is zero (or a reload is requested through $C001), this value
        // will be copied to the IRQ counter at the NEXT rising edge of the PPU
        // address, presumably at PPU cycle 260 of the current scanline.
        self.irq_reload = data;
        Wrote
      }
      // IRQ reload ($C001-$DFFF, odd)
      (0xC001..=0xDFFF, true) => {
        // Writing any value to this register reloads the MMC3 IRQ counter at
        // the NEXT rising edge of the PPU address, presumably at PPU cycle 260
        // of the current scanline.
        self.irq_counter = 0x0000;
        Wrote
      }
      // IRQ disable ($E000-$FFFE, even)
      (0xE000..=0xFFFE, false) => {
        // Writing any value to this register will disable MMC3 interrupts AND
        // acknowledge any pending interrupts.
        self.irq_enabled = false;
        Wrote
      }
      // IRQ enable ($E001-$FFFF, odd)
      (0xE001..=0xFFFF, true) => {
        // Writing any value to this register will enable MMC3 interrupts.
        self.irq_enabled = true;
        Wrote
      }
      _ => WSkip,
    }
  }

  fn safe_cpu_read(&self, addr: u16) -> MappedRead {
    let addr = addr as usize;
    match addr {
      0x6000..=0x7FFF => Data(self.ram[(addr - 0x6000) as usize]),
      0x8000..=0x9FFF => RAddr((addr - 0x8000) + self.prg_bank(0)),
      0xA000..=0xBFFF => RAddr((addr - 0xA000) + self.prg_bank(1)),
      0xC000..=0xDFFF => RAddr((addr - 0xC000) + self.prg_bank(2)),
      0xE000..=0xFFFF => RAddr((addr - 0xE000) + self.prg_bank(3)),
      _ => RSkip,
    }
  }

  fn safe_ppu_read(&self, addr: u16) -> MappedRead {
    let addr = addr as usize;
    match addr {
      0x0000..=0x03FF => RAddr((addr - 0x0000) + self.chr_bank(0)),
      0x0400..=0x07FF => RAddr((addr - 0x0400) + self.chr_bank(1)),
      0x0800..=0x0BFF => RAddr((addr - 0x0800) + self.chr_bank(2)),
      0x0C00..=0x0FFF => RAddr((addr - 0x0C00) + self.chr_bank(3)),
      0x1000..=0x13FF => RAddr((addr - 0x1000) + self.chr_bank(4)),
      0x1400..=0x17FF => RAddr((addr - 0x1400) + self.chr_bank(5)),
      0x1800..=0x1BFF => RAddr((addr - 0x1800) + self.chr_bank(6)),
      0x1C00..=0x1FFF => RAddr((addr - 0x1C00) + self.chr_bank(7)),
      _ => RSkip,
    }
  }

  fn scanline_complete(&mut self) {
    if self.irq_counter == 0 {
      self.irq_counter = self.irq_reload;
    } else {
      self.irq_counter -= 1;
    }

    if self.irq_counter == 0 && self.irq_enabled {
      self.irq_active = true;
    }
  }

  fn irq_active(&mut self) -> bool {
    self.irq_active
  }

  fn irq_clear(&mut self) {
    self.irq_active = false;
  }

  fn mirroring(&self) -> Option<Mirroring> {
    self.mirroring
  }
}
