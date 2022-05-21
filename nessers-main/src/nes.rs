use crate::apu::Apu;
use crate::bus::Bus;
use crate::bus_device::BusDevice;
use crate::cart::Cart;
use crate::cpu6502::Cpu;
use crate::cpu6502::StatusFlag::*;
use crate::disassemble::DisassembledOperation;
use crate::mirror::Mirror;
use crate::palette::Palette;
use crate::peripherals::Peripherals;
use crate::ppu::Ppu;
use crate::ram::Ram;
use crate::trace::{trace, Trace};
use std::collections::HashSet;

pub struct Nes {
  pub breakpoints: HashSet<u16>,

  pub cpu: Cpu,
  pub ppu: Ppu,
  pub apu: Apu,
  tick: u64,
  ram: Ram,
  ram_mirror: Mirror,
  ppu_registers_mirror: Mirror,
  pub cart: Cart,
  pub addresses_hit: HashSet<u16>,
  pub peripherals: Peripherals,

  dma_page: u8,
  dma_addr: u8,
  dma_data: u8,

  dma_active: bool,
  dma_dummy: bool,
}

impl Nes {
  pub fn new(cart_filename: &str, palette_filename: &str) -> Result<Nes, &'static str> {
    let cpu = Cpu::new();

    // 2K internal RAM, mirrored to 8K
    let ram = Ram::new(0x0000, 2 * 1024);
    let ram_mirror = Mirror::new(0x0000, 8 * 1024);

    // PPU Registers, mirrored for 8K
    let ppu = Ppu::new(Palette::from_file(palette_filename)?);
    let ppu_registers_mirror = Mirror::new(0x2000, 8 * 1024);

    let apu = Apu::new();

    let cart = Cart::from_file(cart_filename)?;

    Ok(Nes {
      tick: 0,
      cpu,
      ppu,
      apu,
      cart,
      ram_mirror,
      ram,
      ppu_registers_mirror,
      addresses_hit: HashSet::new(),
      peripherals: Peripherals::new(),
      breakpoints: HashSet::new(),

      dma_page: 0x00,
      dma_addr: 0x00,
      dma_data: 0x00,

      dma_active: false,
      dma_dummy: true,
    })
  }

  pub fn clock(&mut self) {
    self.ppu.clock(&self.cart);
    self.apu.clock();
    if self.tick % 3 == 0 {
      if self.dma_active {
        if self.dma_dummy {
          if self.tick % 2 == 1 {
            self.dma_dummy = false;
          }
        } else {
          if self.tick % 2 == 0 {
            self.dma_data =
              self.cpu_read((self.dma_page as u16) << 8 | ((self.dma_addr as u16) & 0x00FF));
          } else {
            self.ppu.set_oam_data(self.dma_addr, self.dma_data);
            self.dma_addr = self.dma_addr.wrapping_add(1);
            if self.dma_addr == 0x00 {
              self.dma_active = false;
              self.dma_dummy = true;
            }
          }
        }
        // self.dma_active = false;
      } else {
        self.addresses_hit.insert(self.cpu.pc);
        // Is there a shorthand way to run a method on a field by cloning it and
        // replacing its value with the cloned object?
        let cpu = &mut self.cpu.clone();
        cpu.clock(self);
        self.cpu = *cpu;
      }
    }

    if self.ppu.nmi {
      self.ppu.nmi = false;
      let cpu = &mut self.cpu.clone();
      cpu.sig_nmi(self);
      self.cpu = *cpu;
    }

    self.tick += 1;
  }

  pub fn step(&mut self) {
    self.step_with_callback(|_| {})
  }

  pub fn step_with_callback<F>(&mut self, mut callback: F)
  where
    F: FnMut(&mut Self),
  {
    loop {
      callback(self);

      self.clock();
      if self.tick % 3 == 1 && self.cpu.cycles_left == 0 {
        return;
      }
    }
  }

  pub fn frame(&mut self) -> bool {
    loop {
      self.clock();

      // Only breaks on CPU instruction step boundaries; similar to running
      // `step()`:
      if self.tick % 3 == 1 && self.cpu.cycles_left == 0 && self.breakpoints.contains(&self.cpu.pc)
      {
        return true;
      }

      if self.ppu.frame_complete == true {
        return false;
      }
    }
  }

  pub fn break_at(&mut self, addr: &Vec<u16>) {
    loop {
      self.step();
      if addr.contains(&self.cpu.pc) {
        println!("Broke at {:04X}", self.cpu.pc);
        return;
      }
    }
  }

  pub fn reset(&mut self) {
    let cpu = &mut self.cpu.clone();
    cpu.sig_reset(self);
    self.cpu = *cpu;
  }

  pub fn trace(&self) -> String {
    // Example:
    // ```
    // C000  4C F5 C5  JMP $C5F5                       A:00 X:00 Y:00 P:24 SP:FD PPU:  0, 21 CYC:7
    // ^^^^  ^^-^^-^^  ^^^-^^^^^                         ^^   ^^   ^^   ^^    ^^ ^^^^^^^^^^^^^^^^^
    // pc | inst data | disassembled inst              | a  | x  | y|status|stack_pointer| Discarded, for now
    // ```

    let trace = trace(self, self.cpu.pc);
    print_trace(trace)
  }

  // BEGIN ------ Hacky? Helper functions to avoid ugly manual dyn cast -------

  pub fn cpu_read(&mut self, addr: u16) -> u8 {
    (self as &mut dyn Bus<Cpu>).read(addr)
  }

  pub fn cpu_write(&mut self, addr: u16, data: u8) {
    (self as &mut dyn Bus<Cpu>).write(addr, data)
  }

  pub fn cpu_read16(&mut self, addr: u16) -> u16 {
    (self as &mut dyn Bus<Cpu>).read16(addr)
  }

  pub fn safe_cpu_read(&self, addr: u16) -> u8 {
    (self as &dyn Bus<Cpu>).safe_read(addr)
  }
  pub fn safe_cpu_read16(&self, addr: u16) -> u16 {
    (self as &dyn Bus<Cpu>).safe_read16(addr)
  }

  // END -------- Hacky? Helper functions to avoid ugly manual dyn cast -------
}

/// The CPU's Bus
impl Bus<Cpu> for Nes {
  fn safe_read(&self, addr: u16) -> u8 {
    match None // Hehe, using None here just for formatting purposes:
      .or(self.cart.cpu_mapper.read(addr))
      .or(self.ram_mirror.safe_read(&self.ram, addr, &self.cart))
    {
      Some(data) => data,
      None => 0x00,
    }
  }

  fn read(&mut self, addr: u16) -> u8 {
    match None // Hehe, using None here just for formatting purposes:
      .or(self.cart.cpu_mapper.read(addr))
      .or(self.peripherals.read(addr, &self.cart))
      .or(self.ram_mirror.read(&mut self.ram, addr, &self.cart))
      .or(
        self
          .ppu_registers_mirror
          .read(&mut self.ppu, addr, &self.cart),
      ) {
      Some(data) => data,
      None => 0x00,
    }
  }

  fn write(&mut self, addr: u16, data: u8) {
    None // Hehe, using None here just for formatting purposes:
      .or_else(|| self.cart.cpu_mapper.write(addr, data))
      .or_else(|| self.apu.cpu_write(addr, data))
      .or_else(|| {
        // Writing to 0x4014
        //
        // https://www.nesdev.org/wiki/PPU_registers#OAMDMA
        if addr == 0x4014 {
          self.dma_page = data;
          self.dma_addr = 0x00;
          self.dma_active = true;
          return Some(());
        }

        None
      })
      .or_else(|| self.peripherals.write(addr, data, &mut self.cart))
      .or_else(|| {
        self
          .ram_mirror
          .write(&mut self.ram, addr, data, &mut self.cart)
      })
      .or_else(|| {
        self
          .ppu_registers_mirror
          .write(&mut self.ppu, addr, data, &mut self.cart)
      });
  }
}

/// The PPU's Bus
// impl Bus<Ppu> for Nes {
//   fn safe_read(&self, _: u16) -> u8 {
//     todo!()
//   }

//   fn read(&mut self, addr_: u16) -> u8 {
//     let addr = addr_ & 0x3FFF;
//     match None // Hehe, using None here just for formatting purposes:
//       .or(self.cart.ppu_mapper.read(addr))
//       .or(Some(self.ppu.ppu_read(addr, &self.cart)))
//     {
//       Some(data) => data,
//       None => 0x00,
//     }
//   }

//   fn write(&mut self, addr_: u16, data: u8) {
//     let addr = addr_ & 0x3FFF;

//     None // Hehe, using None here just for formatting purposes:
//       .or_else(|| self.cart.ppu_mapper.write(addr, data))
//       .or_else(|| Some(self.ppu.ppu_write(addr, data, &self.cart)));
//   }
// }

pub fn print_trace(trace: Trace) -> String {
  let cpu = trace.cpu;
  let disassembled: DisassembledOperation = trace.into();

  let instruction_data = disassembled
    .data
    .iter()
    .map(|byte| format!("{:02X}", byte))
    .collect::<Vec<String>>()
    .join(" ");

  format!(
    "{:04X}  {:<8} {}{} {:<26}  A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X}",
    disassembled.addr,
    instruction_data,
    if disassembled.undocumented { "*" } else { " " },
    disassembled.instruction_name,
    disassembled.params,
    cpu.a,
    cpu.x,
    cpu.y,
    cpu.status,
    cpu.s
  )
}

pub fn print_trace2(trace: Trace) -> String {
  let cpu = trace.cpu;
  let disassembled: DisassembledOperation = trace.into();

  let instruction_data = disassembled
    .data
    .iter()
    .map(|byte| format!("{:02X}", byte))
    .collect::<Vec<String>>()
    .join(" ");

  #[rustfmt::skip]
  let status_string = format!("{}{}{}{}{}{}{}{}", 
    if cpu.get_status(Negative) != 0 { "N" } else { "n"},
    if cpu.get_status(Overflow) != 0 { "V" } else { "v"},
    if cpu.get_status(Unused) != 0 { "U" } else { "u"},
    if cpu.get_status(Break) != 0 { "B" } else { "b"},
    if cpu.get_status(DecimalMode) != 0 { "D" } else { "d"},
    if cpu.get_status(DisableInterrupts) != 0 { "I" } else { "i"},
    if cpu.get_status(Zero) != 0 { "Z" } else { "z"},
    if cpu.get_status(Carry) != 0 { "C" } else { "c"},
  );

  format!(
    "{:04X}  {:<8} {}{} {:<26}  A:{:02X} {:08b} X:{:02X} Y:{:02X} SP:{:02X} {}",
    disassembled.addr,
    instruction_data,
    if disassembled.undocumented { "*" } else { " " },
    disassembled.instruction_name,
    disassembled.params,
    cpu.a,
    cpu.a,
    cpu.x,
    cpu.y,
    cpu.s,
    status_string,
  )
}

#[cfg(test)]
mod tests {
  use crate::{
    cart::{FLAG_HAS_RAM, FLAG_MIRRORING},
    palette::Color,
  };
  use pretty_assertions::assert_eq;
  use std::{
    fs::File,
    io::{self, BufRead},
    path::Path,
  };

  use crate::cpu6502::AddressingMode::*;
  use crate::cpu6502::Instruction::*;

  use super::*;

  // The output is wrapped in a Result to allow matching on errors
  // Returns an Iterator to the Reader of the lines of the file.
  fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
  where
    P: AsRef<Path>,
  {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
  }

  fn make_test_nes() -> Nes {
    let mut cart_data = vec![
      0x4E,                                   // N
      0x45,                                   // E
      0x53,                                   // S
      0x1A,                                   // EOF
      0x01,                                   // 1 * 16K PRG
      0x01,                                   // 1 * 8K CHR
      (0x00 | FLAG_MIRRORING | FLAG_HAS_RAM), // Lower nybble of mapper code + Flags
      (0x00 | 0x01),                          // Upper nybble of mapper code + iNES version
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
    cart_data.resize(16 + 0 + 16 * 1024, 0x42);
    // Fill CHR with 0x43
    cart_data.resize(16 + 0 + 16 * 1024 + 8 * 1024, 0x43);
    let cpu = Cpu::new();

    // 2K internal RAM, mirrored to 8K
    let ram = Ram::new(0x0000, 2 * 1024);
    let ram_mirror = Mirror::new(0x0000, 8 * 1024);

    // PPU Registers, mirrored for 8K
    let ppu = Ppu::new(Palette {
      colors: [Color { r: 0, g: 0, b: 0 }; 64],
      map: [0x00; 32],
    });
    let ppu_registers_mirror = Mirror::new(0x2000, 8 * 1024);

    let apu = Apu::new();

    let cart = Cart::new(&cart_data).unwrap();

    Nes {
      tick: 0,
      cpu,
      ppu,
      apu,
      cart,
      ram_mirror,
      ram,
      ppu_registers_mirror,
      addresses_hit: HashSet::new(),
      peripherals: Peripherals::new(),
      breakpoints: HashSet::new(),
      dma_page: 0x00,
      dma_addr: 0x00,
      dma_data: 0x00,
      dma_active: false,
      dma_dummy: true,
    }
  }

  fn debug_line_test(prog_data: &Vec<u8>, cpu: Cpu, expected_output: &'static str) {
    let mut nes = make_test_nes();
    nes.cpu = cpu;

    for i in 0..prog_data.len() {
      nes.cpu_write(nes.cpu.pc + (i as u16), prog_data[i]);
    }

    assert_eq!(nes.trace(), expected_output);
  }

  #[test]
  fn test_get_debug_line() {
    debug_line_test(
      &vec![0xF0, 0x04],
      Cpu {
        pc: 0xC7ED,
        a: 0x6F,
        x: 0x00,
        y: 0x00,
        status: 0x6F,
        s: 0xFB,
        cycles_left: 0,
      },
      "C7ED  F0 04     BEQ $C7F3                       A:6F X:00 Y:00 P:6F SP:FB",
    );
    debug_line_test(
      &vec![0xA9, 0x70],
      Cpu {
        pc: 0xD082,
        a: 0xF5,
        x: 0x00,
        y: 0x5F,
        status: 0x65,
        s: 0xFB,
        cycles_left: 0,
      },
      "D082  A9 70     LDA #$70                        A:F5 X:00 Y:5F P:65 SP:FB",
    );

    // debug_line_test(
    //   &vec![0x8D, 0x00, 0x03],
    //   Cpu {
    //     pc: 0xD084,
    //     a: 0x70,
    //     x: 0x00,
    //     y: 0x5F,
    //     status: 0x65,
    //     s: 0xFB,
    //     cycles_left: 0,
    //   },
    //   "D084  8D 00 03  STA $0300 = EF                  A:70 X:00 Y:5F P:65 SP:FB",
    // )
  }

  #[test]
  fn test_format_trace() {
    let mut nes = make_test_nes();
    nes.cpu_write(100, 0xa2);
    nes.cpu_write(101, 0x01);
    nes.cpu_write(102, 0xca);
    nes.cpu_write(103, 0x88);
    nes.cpu_write(104, 0x00);
    nes.cpu = Cpu::new();
    nes.cpu.pc = 100;
    nes.cpu.a = 1;
    nes.cpu.x = 2;
    nes.cpu.y = 3;

    assert_eq!(
      "0064  A2 01     LDX #$01                        A:01 X:02 Y:03 P:24 SP:FD",
      nes.trace()
    );
    nes.step();

    assert_eq!(
      "0066  CA        DEX                             A:01 X:01 Y:03 P:24 SP:FD",
      nes.trace()
    );
    nes.step();

    assert_eq!(
      "0067  88        DEY                             A:01 X:00 Y:03 P:26 SP:FD",
      nes.trace()
    );
    nes.step();
  }

  #[test]
  fn test_format_mem_access() {
    let mut nes = make_test_nes();
    // ORA ($33), Y
    nes.cpu_write(100, 0x11);
    nes.cpu_write(101, 0x33);

    //data
    nes.cpu_write(0x0033, 00);
    nes.cpu_write(0x0034, 04);

    //target cell
    nes.cpu_write(0x0400, 0xAA);

    nes.cpu = Cpu::new();
    nes.cpu.pc = 100;
    nes.cpu.y = 0;

    assert_eq!(
      "0064  11 33     ORA ($33),Y = 0400 @ 0400 = AA  A:00 X:00 Y:00 P:24 SP:FD",
      nes.trace()
    );
  }
  // We're jumping into testing things like the PPU without really validating
  // our CPU.
  //
  // Let's write a test that uses `nestest.nes` to validate CPU behavior (or at
  // least provides a snapshot we can keep track of).

  #[test]
  fn nestest() {
    let mut nes = match Nes::new(
      "nessers-main/src/test_fixtures/nestest.nes",
      "nessers-main/src/test_fixtures/ntscpalette.pal",
    ) {
      Ok(n) => n,
      Err(msg) => panic!("{}", msg),
    };

    nes.cpu.pc = 0xC000;
    let mut line_num = 0;
    // First few traces:
    read_lines("nessers-main/src/test_fixtures/nestest.log")
      .unwrap()
      .for_each(|line| {
        line_num += 1;
        // After these lines we're dealing with APU functionality which isn't
        // implemented yet:
        if line_num > 8980 {
          return;
        }

        // We strip the last part which contains PPU state and cycle count stuff
        // which we're not yet ready to test:
        assert_eq!(nes.trace(), line.unwrap()[0..73]);
        nes.step();
      });
  }

  // Meh. Wild goose chase.
  //
  // #[test]
  // fn smbtest() {
  //   #[derive(Debug, PartialEq)]
  //   struct SMBTest {
  //     line_num: usize,
  //     trace: String,
  //   }
  //   let mut nes = match Nes::new(
  //     // You'll need to provide your own backup of Super Mario Bros here:
  //     "nessers-main/src/test_fixtures/smb.nes",
  //     "nessers-main/src/test_fixtures/ntscpalette.pal",
  //   ) {
  //     Ok(n) => n,
  //     Err(msg) => panic!("{}", msg),
  //   };

  //   // Shouldn't this happen automatically?
  //   nes.cpu.pc = 0x8000;

  //   let addrs_to_skip: Vec<u16> = vec![
  //     0x800D, 0x800A, 0x8012, 0x800F, 0x801B, 0x801F, 0x8020, 0x801D, 0x8018,
  //   ];
  //   let line_nums_to_skip: Vec<usize> = vec![
  //     // 25569,
  //     // 25570,
  //     // 25571,
  //     // 25572,
  //   ];

  //   // First few traces:
  //   let lines: Vec<String> = read_lines("nessers-main/src/test_fixtures/smb.log")
  //     .unwrap()
  //     .map(|line| line.unwrap())
  //     .collect();

  //   let mut line_num = 0;

  //   while line_num < lines.len() {
  //     while addrs_to_skip.contains(&nes.cpu.pc) {
  //       nes.step();
  //     }

  //     let mut fceux_trace;
  //     loop {
  //       line_num += 1;
  //       match from_fceux_trace(&lines[line_num - 1]) {
  //         Ok(trace) => {
  //           fceux_trace = trace;
  //           if !addrs_to_skip.contains(&fceux_trace.cpu.pc) {
  //             break;
  //           }
  //         }
  //         Err(_) => {
  //           panic!("Failed on line {}", line_num);
  //         }
  //       }
  //     }

  //     let trace = trace(&nes, nes.cpu.pc);
  //     nes.step();

  //     if line_nums_to_skip.contains(&line_num) {
  //       continue;
  //     }

  //     assert_eq!(
  //       SMBTest {
  //         trace: print_trace2(trace),
  //         line_num
  //       },
  //       SMBTest {
  //         trace: print_trace2(fceux_trace),
  //         line_num
  //       },
  //     );
  //   }
  // }

  fn from_fceux_trace(string: &str) -> Result<Trace, std::num::ParseIntError> {
    // $8000: 78       SEIA:00 X:00 Y:00 S:FD P:nvubdIzc
    // $8001: D8       CLDA:00 X:00 Y:00 S:FD P:nvubdIzc
    // $8002: A9 10    LDA #$10A:00 X:00 Y:00 S:FD P:nvubdIzc
    // $8004: 8D 00 20 STA $2000 = #$00A:10 X:00 Y:00 S:FD P:nvubdIzc
    // $8007: A2 FF    LDX #$FFA:10 X:00 Y:00 S:FD P:nvubdIzc
    // $8009: 9A       TXSA:10 X:FF Y:00 S:FD P:NvubdIzc
    let mut cpu = Cpu::new();

    // $8000: 78       SEIA:00 X:00 Y:00 S:FD P:nvubdIzc
    //  ^^^^
    cpu.pc = u16::from_str_radix(&string[1..5], 16)?;

    let mut data: Vec<u8> = vec![];
    // $8004: 8D 00 20 STA $2000 = #$00A:10 X:00 Y:00 S:FD P:nvubdIzc
    //        ^^ ^^ ^^
    for i in 0..3 {
      let read = u8::from_str_radix(&string[(7 + i * 3)..(7 + i * 3 + 2)], 16);
      match read {
        Ok(byte) => data.push(byte),
        Err(_) => {
          break;
        }
      }
    }

    // $8000: 78       SEIA:00 X:00 Y:00 S:FD P:nvubdIzc
    //                 ^^^
    let instruction = match &string[16..19] {
      "ADC" => ADC,
      "AND" => AND,
      "ASL" => ASL,
      "BCC" => BCC,
      "BCS" => BCS,
      "BEQ" => BEQ,
      "BIT" => BIT,
      "BMI" => BMI,
      "BNE" => BNE,
      "BPL" => BPL,
      "BRK" => BRK,
      "BVC" => BVC,
      "BVS" => BVS,
      "CLC" => CLC,
      "CLD" => CLD,
      "CLI" => CLI,
      "CLV" => CLV,
      "CMP" => CMP,
      "CPX" => CPX,
      "CPY" => CPY,
      "DEC" => DEC,
      "DEX" => DEX,
      "DEY" => DEY,
      "EOR" => EOR,
      "INC" => INC,
      "INX" => INX,
      "INY" => INY,
      "JMP" => JMP,
      "JSR" => JSR,
      "LDA" => LDA,
      "LDX" => LDX,
      "LDY" => LDY,
      "LSR" => LSR,
      "NOP" => NOP,
      "ORA" => ORA,
      "PHA" => PHA,
      "PHP" => PHP,
      "PLA" => PLA,
      "PLP" => PLP,
      "ROL" => ROL,
      "ROR" => ROR,
      "RTI" => RTI,
      "RTS" => RTS,
      "SBC" => SBC,
      "SEC" => SEC,
      "SED" => SED,
      "SEI" => SEI,
      "STA" => STA,
      "STX" => STX,
      "STY" => STY,
      "TAX" => TAX,
      "TAY" => TAY,
      "TSX" => TSX,
      "TXA" => TXA,
      "TXS" => TXS,
      "TYA" => TYA,
      "LAX" => LAX,
      "SAX" => SAX,
      "DCP" => DCP,
      "ISB" => ISB,
      "SLO" => SLO,
      "RLA" => RLA,
      "SRE" => SRE,
      "RRA" => RRA,
      _ => NOP,
    };

    let mut param: u8 = 0x00;
    let mut addr: u16 = 0x0000;
    let mut addr_abs: u16 = 0x0000;

    let flags_start: usize;
    // If our next char is "A" then we are using implied addressing mode; the
    // "A" is the A register label.
    //
    // $8000: 78       SEIA:00 X:00 Y:00 S:FD P:nvubdIzc
    //                    ^
    let addressing_mode = if &string[19..20] == "A" {
      flags_start = 19;
      IMP
    } else {
      // $8002: A9 10    LDA #$10A:00 X:00 Y:00 S:FD P:nvubdIzc
      //                     ^
      match &string[20..21] {
        "#" => {
          // $8002: A9 10    LDA #$10A:00 X:00 Y:00 S:FD P:nvubdIzc
          //                       ^^
          param = u8::from_str_radix(&string[22..24], 16)?;
          // $8002: A9 10    LDA #$10A:00 X:00 Y:00 S:FD P:nvubdIzc
          //                         ^
          flags_start = 24;
          IMM
        }
        "$" => {
          if instruction == JSR {
            // $802B: 20 CC 90 JSR $90CCA:FF X:05 Y:FE S:FF P:NvubdIzC
            //                          ^
            flags_start = 25;
            ABS
          } else if data.len() == 3 {
            // $8004: 8D 00 20 STA $2000 = #$00A:10 X:00 Y:00 S:FD P:nvubdIzc
            //                      ^^^^
            addr = u16::from_str_radix(&string[21..25], 16)?;
            // $8018: BD D7 07 LDA $07D7,X @ $07DC = #$FFA:90 X:05 Y:FE S:FF P:nvubdIzc
            //                          ^
            if &string[25..26] == "," {
              // $8018: BD D7 07 LDA $07D7,X @ $07DC = #$FFA:90 X:05 Y:FE S:FF P:nvubdIzc
              //                                ^^^^
              addr_abs = u16::from_str_radix(&string[31..35], 16)?;
              // $8018: BD D7 07 LDA $07D7,X @ $07DC = #$FFA:90 X:05 Y:FE S:FF P:nvubdIzc
              //                                           ^
              flags_start = 42;
              match &string[26..27] {
                "X" => ABX,
                "Y" => ABY,
                _ => panic!("Unexpected 'ADDR,{}'", &string[26..27]),
              }
            } else {
              // $8004: 8D 00 20 STA $2000 = #$00A:10 X:00 Y:00 S:FD P:nvubdIzc
              //                                 ^
              flags_start = 32;
              ABS
            }
          } else {
            // $800D: 10 FB    BPL $800AA:10 X:FF Y:00 S:FF P:nvubdIzc
            //                      ^^^^
            addr_abs = u16::from_str_radix(&string[21..25], 16)?;
            // $800D: 10 FB    BPL $800AA:10 X:FF Y:00 S:FF P:nvubdIzc
            //                          ^
            flags_start = 25;
            REL
          }
        }
        _ => {
          flags_start = 9999;
          ZPX
        }
      }
    };

    // ___________A:00 X:00 Y:00 S:FD P:nvubdIzc
    // flags_start| ^^
    cpu.a = u8::from_str_radix(&string[(flags_start + 2)..(flags_start + 4)], 16)?;

    // ___________A:00 X:00 Y:00 S:FD P:nvubdIzc
    // flags_start|      ^^
    cpu.x = u8::from_str_radix(&string[(flags_start + 7)..(flags_start + 9)], 16)?;

    // ___________A:00 X:00 Y:00 S:FD P:nvubdIzc
    // flags_start|           ^^
    cpu.y = u8::from_str_radix(&string[(flags_start + 12)..(flags_start + 14)], 16)?;

    // ___________A:00 X:00 Y:00 S:FD P:nvubdIzc
    // flags_start|                ^^
    cpu.s = u8::from_str_radix(&string[(flags_start + 17)..(flags_start + 19)], 16)?;

    // ___________A:00 X:00 Y:00 S:FD P:nvubdIzc
    // flags_start|                     ^
    let s = flags_start + 22;
    cpu.set_status(Negative, &string[(s + 0)..(s + 1)] == "N");
    cpu.set_status(Overflow, &string[(s + 1)..(s + 2)] == "V");
    // Looks like FCEUX always keeps this un-set but our CPU emulation follows a
    // different spec I guess?
    //
    // cpu.set_status(Unused, &string[(s + 2)..(s + 3)] == "U");
    cpu.set_status(Break, &string[(s + 3)..(s + 4)] == "B");
    cpu.set_status(DecimalMode, &string[(s + 4)..(s + 5)] == "D");
    cpu.set_status(DisableInterrupts, &string[(s + 5)..(s + 6)] == "I");
    cpu.set_status(Zero, &string[(s + 6)..(s + 7)] == "Z");
    cpu.set_status(Carry, &string[(s + 7)..(s + 8)] == "C");

    Ok(Trace {
      cpu,
      instruction,
      addressing_mode,
      // TODO
      undocumented: false,
      data,
      param,
      param_expanded: 0x00,
      addr,
      addr_abs,
      data_at: 0x00,
    })
  }

  #[test]
  fn test_from_fceux_trace() {
    {
      let mut cpu = Cpu::new();
      cpu.pc = 0x8000;
      assert_eq!(
        from_fceux_trace("$8000: 78       SEIA:00 X:00 Y:00 S:FD P:nvubdIzc ").unwrap(),
        Trace {
          cpu,
          instruction: SEI,
          addressing_mode: IMP,
          undocumented: false,
          data: vec![0x78],
          param: 0x00,
          param_expanded: 0x00,
          addr: 0x00,
          addr_abs: 0x00,
          data_at: 0x00,
        }
      );
    }

    {
      let mut cpu = Cpu::new();
      cpu.pc = 0x8000;
      cpu.a = 0x42;
      cpu.x = 0xF5;
      cpu.y = 0xA9;
      cpu.s = 0xFD;
      assert_eq!(
        from_fceux_trace("$8000: 78       SEIA:42 X:F5 Y:A9 S:FD P:nvubdIzc ").unwrap(),
        Trace {
          cpu,
          instruction: SEI,
          addressing_mode: IMP,
          undocumented: false,
          data: vec![0x78],
          param: 0x00,
          param_expanded: 0x00,
          addr: 0x00,
          addr_abs: 0x00,
          data_at: 0x00,
        }
      );
    }

    {
      let mut cpu = Cpu::new();
      cpu.pc = 0x8002;
      assert_eq!(
        from_fceux_trace("$8002: A9 10    LDA #$10A:00 X:00 Y:00 S:FD P:nvubdIzc ").unwrap(),
        Trace {
          cpu,
          instruction: LDA,
          addressing_mode: IMM,
          undocumented: false,
          data: vec![0xA9, 0x10],
          param: 0x10,
          param_expanded: 0x00,
          addr: 0x00,
          addr_abs: 0x00,
          data_at: 0x00,
        }
      );
    }

    {
      let mut cpu = Cpu::new();
      cpu.pc = 0x8004;
      cpu.a = 0x10;
      assert_eq!(
        from_fceux_trace("$8004: 8D 00 20 STA $2000 = #$00A:10 X:00 Y:00 S:FD P:nvubdIzc ")
          .unwrap(),
        Trace {
          cpu,
          instruction: STA,
          addressing_mode: ABS,
          undocumented: false,
          data: vec![0x8D, 0x00, 0x20],
          param: 0x00,
          param_expanded: 0x00,
          addr: 0x2000,
          addr_abs: 0x00,
          data_at: 0x00,
        }
      );
    }

    {
      let mut cpu = Cpu::new();
      cpu.pc = 0x800D;
      cpu.a = 0x10;
      cpu.x = 0xFF;
      cpu.s = 0xFF;
      assert_eq!(
        from_fceux_trace("$800D: 10 FB    BPL $800AA:10 X:FF Y:00 S:FF P:nvubdIzc ").unwrap(),
        Trace {
          cpu,
          instruction: BPL,
          addressing_mode: REL,
          undocumented: false,
          data: vec![0x10, 0xFB],
          param: 0x00,
          param_expanded: 0x00,
          addr: 0x0000,
          addr_abs: 0x800A,
          data_at: 0x00,
        }
      );
    }

    {
      let mut cpu = Cpu::new();
      cpu.pc = 0x90D4;
      cpu.a = 0x00;
      cpu.x = 0x02;
      cpu.y = 0x72;
      cpu.s = 0xFD;
      assert_eq!(
        from_fceux_trace("$90D4: E0 01    CPX #$01A:00 X:02 Y:72 S:FD P:nvubdIzc ").unwrap(),
        Trace {
          cpu,
          instruction: CPX,
          addressing_mode: IMM,
          undocumented: false,
          data: vec![0xE0, 0x01],
          param: 0x01,
          param_expanded: 0x00,
          addr: 0x0000,
          addr_abs: 0x0000,
          data_at: 0x00,
        }
      );
    }

    {
      let mut cpu = Cpu::new();
      cpu.pc = 0x8001;
      cpu.s = 0xFD;
      assert_eq!(
        from_fceux_trace("$8001: D8       CLDA:00 X:00 Y:00 S:FD P:nvubdIzc ").unwrap(),
        Trace {
          cpu,
          instruction: CLD,
          addressing_mode: IMP,
          undocumented: false,
          data: vec![0xD8],
          param: 0x00,
          param_expanded: 0x00,
          addr: 0x0000,
          addr_abs: 0x0000,
          data_at: 0x00,
        }
      );
    }

    {
      let mut cpu = Cpu::new();
      cpu.pc = 0x802B;
      cpu.a = 0xFF;
      cpu.x = 0x05;
      cpu.y = 0xFE;
      cpu.s = 0xFF;
      cpu.set_status(Negative, true);
      cpu.set_status(DisableInterrupts, true);
      cpu.set_status(Carry, true);
      assert_eq!(
        from_fceux_trace("$802B: 20 CC 90 JSR $90CCA:FF X:05 Y:FE S:FF P:NvubdIzC ").unwrap(),
        Trace {
          cpu,
          instruction: JSR,
          addressing_mode: ABS,
          undocumented: false,
          data: vec![0x20, 0xCC, 0x90],
          param: 0x00,
          param_expanded: 0x00,
          addr: 0x0000,
          addr_abs: 0x0000,
          data_at: 0x00,
        }
      );
    }
  }
}
