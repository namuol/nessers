#![allow(unused_comparisons)]

use super::*;

pub struct M069 {
  num_prg_banks: usize,
  num_chr_banks: usize,
  command: u8,
  param: u8,
  prg_bank: [u8; 4],
  chr_bank: [u8; 8],
  ram_bank: u8,
  ram_select: bool,
  ram: [u8; 512 * 1024],
  mirroring: Option<Mirroring>,

  irq_control: u8,
  irq_counter: u16,
  irq_active: bool,
}

impl M069 {
  pub fn new(num_prg_banks: usize, num_chr_banks: usize) -> Self {
    M069 {
      num_prg_banks,
      num_chr_banks,
      command: 0x00,
      param: 0x00,
      prg_bank: [0x00; 4],
      chr_bank: [0x00; 8],
      ram_bank: 0x00,
      ram_select: false,
      ram: [0x00; 512 * 1024],
      mirroring: None,
      irq_control: 0x00,
      irq_counter: 0x0000,
      irq_active: false,
    }
  }

  // IRQ Control ($D)
  //
  // ```
  // 7  bit  0
  // ---- ----
  // C... ...T
  // |       |
  // |       +- IRQ Enable
  // |           0 = Do not generate IRQs
  // |           1 = Do generate IRQs
  // +-------- IRQ Counter Enable
  //             0 = Disable Counter Decrement
  //             1 = Enable Counter Decrement
  // ```
  fn irq_decrement_enabled(&mut self) -> bool {
    (self.irq_control & 0b1000_0000) != 0
  }

  fn irq_enabled(&mut self) -> bool {
    (self.irq_control & 0b0000_0001) != 0
  }
}

impl Mapper for M069 {
  fn cpu_write(&mut self, addr: u16, data: u8) -> MappedWrite {
    // Configuration of the FME-7 is accomplished by first writing the command
    // number to the Command Register at $8000-9FFF, then writing the command's
    // parameter byte to the Parameter Register at $A000-BFFF.
    match addr {
      0x6000..=0x7FFF => {
        if self.ram_select {
          self.ram[((addr as usize) - 0x6000) + (self.ram_bank as usize) * 8 * 1024] = data;
          Wrote
        } else {
          WAddr(((addr as usize) - 0x6000) + (self.prg_bank[0] as usize) * 8 * 1024)
        }
      }
      0x8000..=0x9FFF => {
        // Command Register ($8000-$9FFF)
        //
        // ```
        // 7  bit  0
        // ---- ----
        // .... CCCC
        //      ||||
        //      ++++- The command number to invoke when writing to the Parameter Register
        // ```
        self.command = data & 0b0000_1111;
        // println!("set command: {:02X}", self.command);
        Wrote
      }
      0xA000..=0xBFFF => {
        // Parameter Register ($A000-$BFFF)
        //
        // ```
        // 7  bit  0
        // ---- ----
        // PPPP PPPP
        // |||| ||||
        // ++++-++++- The parameter to use for this command. Writing to this register invokes the command in the Command Register.
        // ```
        self.param = data;
        // println!("run command: {:02X} ({:02X})", self.command, data);
        match self.command {
          0x00..=0x07 => {
            // CHR Bank 0-7 ($0-7)
            //
            // ```
            // 7  bit  0
            // ---- ----
            // BBBB BBBB
            // |||| ||||
            // ++++-++++- The bank number to select for the specified bank.
            // ```
            // println!(
            //   "chr_bank[{}] = {} (of {})",
            //   self.command, data, self.num_chr_banks
            // );
            self.chr_bank[self.command as usize] = data;
            Wrote
          }
          0x08 => {
            // PRG Bank 0 ($8)
            //
            // ```
            // 7  bit  0
            // ---- ----
            // ERbB BBBB
            // |||| ||||
            // ||++-++++- The bank number to select at CPU $6000 - $7FFF
            // |+------- RAM / ROM Select Bit
            // |         0 = PRG ROM
            // |         1 = PRG RAM
            // +-------- RAM Enable Bit (6264 +CE line)
            //           0 = PRG RAM Disabled
            //           1 = PRG RAM Enabled
            // ```

            // The FME-7 has up to 6 bits for PRG banking (512 KiB), though this
            // was never used in a game. The 5A and 5B, however, support only 5
            // (256 KiB)—hence the lowercase 'b' above. The extra address line
            // is instead an audio expansion line, or unused.
            let bank_num = 0b0001_1111 & data;

            self.ram_select = (0b0100_0000 & data) != 0;
            if self.ram_select {
              // println!("ram_bank = {}", bank_num);
              self.ram_bank = bank_num;
            } else {
              // println!("prg_bank[0] = {} (of {})", bank_num, self.num_prg_banks * 2);
              self.prg_bank[0] = bank_num;
            }

            // I'm not bothering with RAM disabling because this is mostly to
            // protect RAM during hardware reset.
            //
            // > Open bus occurs if the RAM / ROM Select Bit is 1 (RAM
            // > selected), but the RAM Enable Bit is 0 (disabled), i.e. any
            // > value in the range $40-$7F. This is a limited form of WRAM
            // > write protection on power-up.

            Wrote
          }
          0x09..=0x0B => {
            // PRG Bank 1-3 ($9-B)
            //
            // ```
            // 7  bit  0
            // ---- ----
            // ..bB BBBB
            //   || ||||
            //   ++-++++- The bank number to select for the specified bank.
            // ```
            let num = (self.command - 0x09) as usize + 1;
            let bank_num = data & 0b0001_1111;
            // println!(
            //   "prg_bank[{}] = {} (of {})",
            //   num,
            //   bank_num,
            //   self.num_prg_banks * 2
            // );
            self.prg_bank[num] = bank_num;
            Wrote
          }
          0x0C => {
            // Name Table Mirroring ($C)
            //
            // These values are the same as MMC1 mirroring modes with the MSB inverted.
            //
            // ```
            // 7  bit  0
            // ---- ----
            // .... ..MM
            //        ||
            //        ++- Mirroring Mode
            //             0 = Vertical
            //             1 = Horizontal
            //             2 = One Screen Mirroring from $2000 ("1ScA")
            //             3 = One Screen Mirroring from $2400 ("1ScB")
            // ```
            self.mirroring = match 0b0000_0011 & data {
              0 => Some(Mirroring::Vertical),
              1 => Some(Mirroring::Horizontal),
              2 => Some(Mirroring::OneScreenLo),
              3 => Some(Mirroring::OneScreenHi),
              _ => None,
            };
            // println!(
            //   "mirroring: {}",
            //   match self.mirroring {
            //     Some(Mirroring::Horizontal) => "Horizontal",
            //     Some(Mirroring::Vertical) => "Vertical",
            //     Some(Mirroring::OneScreenLo) => "OneScreenLo",
            //     Some(Mirroring::OneScreenHi) => "OneScreenHi",
            //     None => "None",
            //   }
            // );
            Wrote
          }
          0xD => {
            // IRQ Control ($D)
            //
            // ```
            // 7  bit  0
            // ---- ----
            // C... ...T
            // |       |
            // |       +- IRQ Enable
            // |           0 = Do not generate IRQs
            // |           1 = Do generate IRQs
            // +-------- IRQ Counter Enable
            //             0 = Disable Counter Decrement
            //             1 = Enable Counter Decrement
            // ```
            // All writes to this register acknowledge an active IRQ.[1] It is
            // not yet known what will happen if this register is written to at
            // the same time as an IRQ would have been generated.
            // println!("irq_control: {:08b}", data);
            self.irq_control = data;
            Wrote
          }
          0xE => {
            // IRQ Counter Low Byte ($E)
            //
            // ```
            // 7  bit  0
            // ---- ----
            // LLLL LLLL
            // |||| ||||
            // ++++-++++- The low eight bits of the IRQ counter
            // ```
            // println!("irq_counter low: {}", data);
            self.irq_counter = (self.irq_counter & 0xFF00) | (data as u16 & 0x00FF);
            Wrote
          }

          0xF => {
            // IRQ Counter High Byte ($F)
            //
            // ```
            // 7  bit  0
            // ---- ----
            // HHHH HHHH
            // |||| ||||
            // ++++-++++- The high eight bits of the IRQ counter
            // ```
            // println!("irq_counter hi: {}", (data as u16) << 8);
            self.irq_counter = (self.irq_counter & 0x00FF) | (((data as u16) << 8) & 0xFF00);
            // println!("irq_counter: {}", self.irq_counter);
            Wrote
          }

          _ => WSkip,
        }
      }
      _ => WSkip,
    }
  }

  fn safe_cpu_read(&self, addr: u16) -> MappedRead {
    let addr = addr as usize;
    match addr {
      // CPU $6000-$7FFF: 8 KB Bankable PRG ROM or PRG RAM
      0x6000..=0x7FFF => {
        if self.ram_select {
          Data(self.ram[(addr - 0x6000) + (self.ram_bank as usize) * 8 * 1024])
        } else {
          RAddr((addr - 0x6000) + (self.prg_bank[0] as usize) * 8 * 1024)
        }
      }
      // CPU $8000-$9FFF: 8 KB Bankable PRG ROM
      0x8000..=0x9FFF => RAddr((addr - 0x8000) + (self.prg_bank[1] as usize) * 8 * 1024),
      // CPU $A000-$BFFF: 8 KB Bankable PRG ROM
      0xA000..=0xBFFF => RAddr((addr - 0xA000) + (self.prg_bank[2] as usize) * 8 * 1024),
      // CPU $C000-$DFFF: 8 KB Bankable PRG ROM
      0xC000..=0xDFFF => RAddr((addr - 0xC000) + (self.prg_bank[3] as usize) * 8 * 1024),
      // CPU $E000-$FFFF: 8 KB PRG ROM, fixed to the last bank of ROM
      //
      // Note: `num_prg_banks` counts 16kbyte banks but we're working with
      // 8kbyte banks, hence the `* 2` below:
      0xE000..=0xFFFF => RAddr((addr - 0xE000) + ((self.num_prg_banks * 2) - 1) * 8 * 1024),
      _ => RSkip,
    }
  }

  fn safe_ppu_read(&self, addr: u16) -> MappedRead {
    let addr = addr as usize;
    match addr {
      // PPU $0000-$03FF: 1 KB Bankable CHR ROM
      0x0000..=0x03FF => RAddr((addr - 0x0000) + (self.chr_bank[0] as usize) * 1024),
      // PPU $0400-$07FF: 1 KB Bankable CHR ROM
      0x0400..=0x07FF => RAddr((addr - 0x0400) + (self.chr_bank[1] as usize) * 1024),
      // PPU $0800-$0BFF: 1 KB Bankable CHR ROM
      0x0800..=0x0BFF => RAddr((addr - 0x0800) + (self.chr_bank[2] as usize) * 1024),
      // PPU $0C00-$0FFF: 1 KB Bankable CHR ROM
      0x0C00..=0x0FFF => RAddr((addr - 0x0C00) + (self.chr_bank[3] as usize) * 1024),
      // PPU $1000-$13FF: 1 KB Bankable CHR ROM
      0x1000..=0x13FF => RAddr((addr - 0x1000) + (self.chr_bank[4] as usize) * 1024),
      // PPU $1400-$17FF: 1 KB Bankable CHR ROM
      0x1400..=0x17FF => RAddr((addr - 0x1400) + (self.chr_bank[5] as usize) * 1024),
      // PPU $1800-$1BFF: 1 KB Bankable CHR ROM
      0x1800..=0x1BFF => RAddr((addr - 0x1800) + (self.chr_bank[6] as usize) * 1024),
      // PPU $1C00-$1FFF: 1 KB Bankable CHR ROM
      0x1C00..=0x1FFF => RAddr((addr - 0x1C00) + (self.chr_bank[7] as usize) * 1024),
      _ => RSkip,
    }
  }

  fn mirroring(&self) -> Option<Mirroring> {
    self.mirroring
  }

  fn clock(&mut self, tick: u64) {
    // The `clock` method is called for every tick of the PPU, of which every
    // third tick is a CPU tick, so here's where we handle CPU clocks:
    if self.irq_decrement_enabled() {
      if (tick % 3) == 0 {
        // The IRQ feature of FME-7 is a CPU cycle counting IRQ generator. When
        // enabled the 16-bit IRQ counter is decremented once per CPU cycle. When
        // the IRQ counter is decremented from $0000 to $FFFF an IRQ is generated.
        // The IRQ line is held low until it is acknowledged.
        if self.irq_counter == 0 {
          if self.irq_enabled() {
            // println!("irq triggered!");
            self.irq_active = true;
          } else {
            // println!("irq not triggered.");
          }
          self.irq_counter = 0xFFFF;
        } else {
          self.irq_counter -= 1;
        }
      }
    }
  }

  fn irq_active(&mut self) -> bool {
    self.irq_active
  }

  fn irq_clear(&mut self) {
    self.irq_active = false;
  }
}
