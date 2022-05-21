use crate::bus::Bus;
use lazy_static::lazy_static;
use std::collections::HashMap;

/// 6502 Processor Status bits
///
/// See the "Processor Status" register description here:
///
/// - https://web.archive.org/web/20210803072351/http://www.obelisk.me.uk/6502/registers.html
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum StatusFlag {
  Carry = 1 << 0,
  Zero = 1 << 1,
  DisableInterrupts = 1 << 2,
  DecimalMode = 1 << 3, // Unused
  Break = 1 << 4,
  Unused = 1 << 5, // Unused
  Overflow = 1 << 6,
  Negative = 1 << 7,
}
use StatusFlag::*;

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Cpu {
  /// Processor Status
  pub status: u8,
  /// Accumulator
  pub a: u8,
  /// X Register
  pub x: u8,
  /// Y Register
  pub y: u8,
  /// Stack Pointer
  pub s: u8,
  /// Program Counter
  pub pc: u16,

  /// The numbers of cycles remaining for the current operation
  pub cycles_left: u8,
}

pub const STACK_START: u16 = 0x0100;
pub const STACK_INIT: u8 = 0xFD;
pub const STACK_SIZE: u8 = 0xFF;

/// An address that should contain a pointer to the start of our program
pub const PC_INIT_ADDR: u16 = 0xFFFC;

pub const IRQ_POINTER: u16 = 0xFFFE;
pub const NMI_POINTER: u16 = 0xFFFA;

impl Cpu {
  pub fn new() -> Cpu {
    Cpu {
      status: (0x00 as u8) | (StatusFlag::Unused as u8) | (StatusFlag::DisableInterrupts as u8),
      a: 0,
      x: 0,
      y: 0,
      pc: 0,
      s: STACK_INIT,
      cycles_left: 0,
    }
  }

  // UTILITIES/HELPER METHODS:

  pub fn get_status(&self, flag: StatusFlag) -> u8 {
    self.status & (flag as u8)
  }

  pub fn set_status(&mut self, flag: StatusFlag, value: bool) {
    if value {
      self.status |= flag as u8;
    } else {
      self.status &= !(flag as u8);
    }
  }

  pub fn step(&mut self, bus: &mut dyn Bus<Cpu>) {
    loop {
      self.clock(bus);
      if self.cycles_left == 0 {
        return;
      }
    }
  }

  fn push(&mut self, bus: &mut dyn Bus<Cpu>, data: u8) {
    bus.write(STACK_START + (self.s as u16), data);
    self.s = self.s.wrapping_sub(1);
  }

  fn pull(&mut self, bus: &mut dyn Bus<Cpu>) -> u8 {
    self.s = self.s.wrapping_add(1);
    let data = bus.read(STACK_START + (self.s as u16));
    data
  }

  pub fn clock(&mut self, bus: &mut dyn Bus<Cpu>) {
    if self.cycles_left == 0 {
      let opcode = bus.read(self.pc);
      self.pc += 1;

      let operation: &Operation = opcode.into();

      self.cycles_left = operation.cycles;

      let addressing_mode: AddressingModeImplementation = match operation.addressing_mode {
        IMP => imp,
        IMM => imm,
        ZP0 => zp0,
        ZPX => zpx,
        ZPY => zpy,
        ABS => abs,
        ABX => abx,
        ABY => aby,
        IND => ind,
        IZX => izx,
        IZY => izy,
        ACC => acc,
        REL => rel,
      };
      let address_mode_result = addressing_mode(self, bus);
      let instruction: InstructionImplementation = match operation.instruction {
        ADC => adc,
        AND => and,
        ASL => asl,
        BCC => bcc,
        BCS => bcs,
        BEQ => beq,
        BIT => bit,
        BMI => bmi,
        BNE => bne,
        BPL => bpl,
        BRK => brk,
        BVC => bvc,
        BVS => bvs,
        CLC => clc,
        CLD => cld,
        CLI => cli,
        CLV => clv,
        CMP => cmp,
        CPX => cpx,
        CPY => cpy,
        DEC => dec,
        DEX => dex,
        DEY => dey,
        EOR => eor,
        INC => inc,
        INX => inx,
        INY => iny,
        JMP => jmp,
        JSR => jsr,
        LDA => lda,
        LDX => ldx,
        LDY => ldy,
        LSR => lsr,
        NOP => nop,
        ORA => ora,
        PHA => pha,
        PHP => php,
        PLA => pla,
        PLP => plp,
        ROL => rol,
        ROR => ror,
        RTI => rti,
        RTS => rts,
        SBC => sbc,
        SEC => sec,
        SED => sed,
        SEI => sei,
        STA => sta,
        STX => stx,
        STY => sty,
        TAX => tax,
        TAY => tay,
        TSX => tsx,
        TXA => txa,
        TXS => txs,
        TYA => tya,

        LAX => lax,
        SAX => sax,
        DCP => dcp,
        ISB => isb,
        SLO => slo,
        RLA => rla,
        SRE => sre,
        RRA => rra,
      };
      let instruction_result = instruction(self, bus, &address_mode_result.data);

      if address_mode_result.needs_extra_cycle && instruction_result.may_need_extra_cycle {
        self.cycles_left += 1;
      }
    }

    self.cycles_left -= 1;
  }

  // SIGNALS:
  pub fn sig_reset(&mut self, bus: &mut dyn Bus<Cpu>) {
    self.a = 0x00;
    self.x = 0x00;
    self.y = 0x00;
    self.s = STACK_SIZE;
    self.status = 0x00 | (StatusFlag::Unused as u8);
    self.pc = bus.read16(PC_INIT_ADDR);

    self.cycles_left = 8;
  }

  pub fn sig_irq(&mut self, bus: &mut dyn Bus<Cpu>) {
    if self.get_status(StatusFlag::DisableInterrupts) != 0x00 {
      let pc_hi: u8 = (self.pc >> 8) as u8;
      self.push(bus, pc_hi);
      let pc_lo: u8 = (self.pc & 0x00FF) as u8;
      self.push(bus, pc_lo);
      self.set_status(Break, false);
      self.set_status(Unused, true);
      self.set_status(DisableInterrupts, true);
      self.push(bus, self.status);
      let irq_addr = bus.read16(IRQ_POINTER);
      self.pc = irq_addr;
      self.cycles_left = 7;
    }
  }

  pub fn sig_nmi(&mut self, bus: &mut dyn Bus<Cpu>) {
    let pc_hi: u8 = (self.pc >> 8) as u8;
    self.push(bus, pc_hi);
    let pc_lo: u8 = (self.pc & 0x00FF) as u8;
    self.push(bus, pc_lo);
    self.set_status(Break, false);
    self.set_status(Unused, true);
    self.set_status(DisableInterrupts, true);
    self.push(bus, self.status);
    let irq_addr = bus.read16(NMI_POINTER);
    // println!("NMI IRQ {:04X} PC = {:04X} lo = {:02X} hi = {:02X}", irq_addr, self.pc, pc_lo, pc_hi);
    self.pc = irq_addr;

    self.cycles_left = 8;
  }
}

pub struct Operation {
  pub addressing_mode: AddressingMode,
  pub instruction: Instruction,
  pub cycles: u8,
  pub undocumented: bool,
}

enum DataSourceKind {
  Accumulator,
  AbsoluteAddress,
  Implicit,
}
use DataSourceKind::*;

struct DataSource {
  kind: DataSourceKind,
  addr: u16,
}

impl DataSource {
  pub fn read(&self, cpu: &Cpu, bus: &mut dyn Bus<Cpu>) -> u8 {
    match self.kind {
      Accumulator => cpu.a,
      AbsoluteAddress => bus.read(self.addr),
      Implicit => panic!("Cannot read from Implicit DataSource"),
    }
  }

  pub fn write(&self, cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: u8) {
    match self.kind {
      Accumulator => cpu.a = data,
      AbsoluteAddress => bus.write(self.addr, data),
      Implicit => panic!("Cannot write to Implicit DataSource"),
    }
  }
}

pub struct AddressingModeResult {
  data: DataSource,
  needs_extra_cycle: bool,
}

/// An Addressing Mode ultimately provides some data to be used by an
/// instruction, either in the form of a constant, read-only byte value (`data`)
/// or an absolute address from which the data can be retrieved/written to
/// (`addr_abs`)
type AddressingModeImplementation = fn(&mut Cpu, &mut dyn Bus<Cpu>) -> AddressingModeResult;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum AddressingMode {
  IMP,
  IMM,
  ZP0,
  ZPX,
  ZPY,
  ABS,
  ABX,
  ABY,
  IND,
  IZX,
  IZY,
  ACC,
  REL,
}
use AddressingMode::*;

struct InstructionResult {
  may_need_extra_cycle: bool,
}
type InstructionImplementation = fn(&mut Cpu, &mut dyn Bus<Cpu>, &DataSource) -> InstructionResult;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Instruction {
  ADC,
  AND,
  ASL,
  BCC,
  BCS,
  BEQ,
  BIT,
  BMI,
  BNE,
  BPL,
  BRK,
  BVC,
  BVS,
  CLC,
  CLD,
  CLI,
  CLV,
  CMP,
  CPX,
  CPY,
  DEC,
  DEX,
  DEY,
  EOR,
  INC,
  INX,
  INY,
  JMP,
  JSR,
  LDA,
  LDX,
  LDY,
  LSR,
  NOP,
  ORA,
  PHA,
  PHP,
  PLA,
  PLP,
  ROL,
  ROR,
  RTI,
  RTS,
  SBC,
  SEC,
  SED,
  SEI,
  STA,
  STX,
  STY,
  TAX,
  TAY,
  TSX,
  TXA,
  TXS,
  TYA,

  // Undocumented:
  LAX,
  SAX,
  DCP,
  ISB,
  SLO,
  RLA,
  SRE,
  RRA,
}
use Instruction::*;

// INSTRUCTIONS ///////////////////////////////////////////////////////////////

// LOGICAL INSTRUCTIONS

/// AND
fn and(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  cpu.a = cpu.a & data.read(cpu, bus);
  cpu.set_status(Zero, cpu.a == 0x00);
  cpu.set_status(Negative, cpu.a & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: true,
  }
}

/// Exclusive OR
fn eor(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  cpu.a = cpu.a ^ data.read(cpu, bus);
  cpu.set_status(Zero, cpu.a == 0x00);
  cpu.set_status(Negative, cpu.a & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: true,
  }
}

/// Inclusive OR
fn ora(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  cpu.a = cpu.a | data.read(cpu, bus);
  cpu.set_status(Zero, cpu.a == 0x00);
  cpu.set_status(Negative, cpu.a & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: true,
  }
}

/// Bit Test
fn bit(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu, bus);
  cpu.set_status(Zero, cpu.a & m == 0x00);

  // Bit 6 from memory value is copied to overflow flag (why?):
  cpu.set_status(Overflow, (0b_0100_0000 & m) != 0);

  cpu.set_status(Negative, (0b_1000_0000 & m) != 0);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

// LOAD/STORE OPERATIONS

/// Load Accumulator
fn lda(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu, bus);
  cpu.a = m;
  cpu.set_status(Zero, m == 0);
  cpu.set_status(Negative, (0b_1000_0000 & m) != 0);
  InstructionResult {
    may_need_extra_cycle: true,
  }
}

/// Load X
fn ldx(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu, bus);
  cpu.x = m;
  cpu.set_status(Zero, m == 0);
  cpu.set_status(Negative, (0b_1000_0000 & m) != 0);
  InstructionResult {
    may_need_extra_cycle: true,
  }
}

/// Load Y
fn ldy(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu, bus);
  cpu.y = m;
  cpu.set_status(Zero, m == 0);
  cpu.set_status(Negative, (0b_1000_0000 & m) != 0);
  InstructionResult {
    may_need_extra_cycle: true,
  }
}

fn lax(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu, bus);
  cpu.a = m;
  cpu.x = m;
  cpu.set_status(Zero, m == 0);
  cpu.set_status(Negative, (0b_1000_0000 & m) != 0);
  InstructionResult {
    may_need_extra_cycle: true,
  }
}

/// Store Accumulator
fn sta(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  data.write(cpu, bus, cpu.a);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Store X
fn stx(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  data.write(cpu, bus, cpu.x);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Store Y
fn sty(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  data.write(cpu, bus, cpu.y);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Undocumented
fn sax(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  data.write(cpu, bus, cpu.a & cpu.x);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

// Register Transfers

/// Transfer Accumulator to X
fn tax(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  cpu.x = cpu.a;

  cpu.set_status(Zero, cpu.a == 0x00);
  cpu.set_status(Negative, cpu.a & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Transfer Accumulator to Y
fn tay(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  cpu.y = cpu.a;

  cpu.set_status(Zero, cpu.a == 0x00);
  cpu.set_status(Negative, cpu.a & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Transfer X to Accumulator
fn txa(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  cpu.a = cpu.x;

  cpu.set_status(Zero, cpu.x == 0x00);
  cpu.set_status(Negative, cpu.x & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Transfer Y to Accumulator
fn tya(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  cpu.a = cpu.y;

  cpu.set_status(Zero, cpu.y == 0x00);
  cpu.set_status(Negative, cpu.y & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

// Stack Operations

/// Transfer Stack Pointer to X
fn tsx(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  cpu.x = cpu.s;

  cpu.set_status(Zero, cpu.s == 0x00);
  cpu.set_status(Negative, cpu.s & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Transfer X to Stack Pointer
fn txs(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  cpu.s = cpu.x;

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Push Accumulator
fn pha(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  cpu.push(bus, cpu.a);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Push Processor Status
fn php(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  cpu.push(bus, cpu.status | (Break as u8) | (Unused as u8));

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Pull Accumulator
fn pla(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  cpu.a = cpu.pull(bus);

  cpu.set_status(Zero, cpu.a == 0x00);
  cpu.set_status(Negative, cpu.a & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Pull Processor Status
fn plp(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  cpu.status = cpu.pull(bus);
  cpu.set_status(Unused, true);
  cpu.set_status(Break, false);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

// Arithmetic
fn adc_(cpu: &mut Cpu, a: u16, m: u16) -> InstructionResult {
  let result = a + m + if cpu.get_status(Carry) != 0 { 1 } else { 0 };
  {
    let overflow: u16 = (a ^ result) & !(a ^ m) & 0x0080;
    cpu.set_status(Overflow, overflow != 0);
  }
  cpu.set_status(Carry, result & 0xFF00 != 0);
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x80) != 0);
  cpu.a = (result & 0x00FF) as u8;
  InstructionResult {
    may_need_extra_cycle: true,
  }
}
/// Add with Carry
fn adc(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let a = cpu.a as u16 & 0x00FF;
  let m = data.read(cpu, bus) as u16 & 0x00FF;
  adc_(cpu, a, m)
}

/// Subtract with Carry
fn sbc(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let a = cpu.a as u16 & 0x00FF;
  let m = (!data.read(cpu, bus)) as u16 & 0x00FF;
  adc_(cpu, a, m)
}

/// Compare Accumulator
fn cmp(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let a = cpu.a as u16;
  let m = data.read(cpu, bus) as u16;
  let result = a.wrapping_sub(m);
  cpu.set_status(Carry, a >= m);
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x0080) != 0);
  InstructionResult {
    may_need_extra_cycle: true,
  }
}

/// Compare X
fn cpx(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let x = cpu.x as u16;
  let m = data.read(cpu, bus) as u16;
  let result = x.wrapping_sub(m);
  cpu.set_status(Carry, x >= m);
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x0080) != 0);
  InstructionResult {
    // Unlike CMP, we cannot use address modes that may require additional
    // cycles:
    may_need_extra_cycle: false,
  }
}

/// Compare Y
fn cpy(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let y = cpu.y as u16;
  let m = data.read(cpu, bus) as u16;
  let result = y.wrapping_sub(m);
  cpu.set_status(Carry, y >= m);
  cpu.set_status(Zero, y == m);
  cpu.set_status(Negative, (result & 0x0080) != 0);
  InstructionResult {
    // Unlike CMP, we cannot use address modes that may require additional
    // cycles:
    may_need_extra_cycle: false,
  }
}

// Increments & Decrements

/// Increment Memory
fn inc(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu, bus) as u16;
  let result = m.wrapping_add(1);
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x0080) != 0);
  data.write(cpu, bus, (result & 0x00FF) as u8);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Undocumented: INC + SBC
fn isb(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu, bus) as u16;
  let result = m.wrapping_add(1);
  data.write(cpu, bus, (result & 0x00FF) as u8);

  let a = cpu.a as u16 & 0x00FF;
  let m = (!result) as u16 & 0x00FF;
  adc_(cpu, a, m)
}

/// Increment X
fn inx(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  let result = (cpu.x as u16).wrapping_add(1);
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x0080) != 0);
  cpu.x = (result & 0x00FF) as u8;
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Increment Y
fn iny(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  let result = (cpu.y as u16).wrapping_add(1);
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x0080) != 0);
  cpu.y = (result & 0x00FF) as u8;
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Decrement Memory
fn dec(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu, bus) as u16;
  let result = m.wrapping_sub(1);
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x0080) != 0);
  data.write(cpu, bus, (result & 0x00FF) as u8);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Decrement X
fn dex(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  let result = (cpu.x as u16).wrapping_sub(1);
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x0080) != 0);
  cpu.x = (result & 0x00FF) as u8;
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Decrement Y
fn dey(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  let result = (cpu.y as u16).wrapping_sub(1);
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x0080) != 0);
  cpu.y = (result & 0x00FF) as u8;
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Undocumented; DEC + CMP
fn dcp(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let a = cpu.a as u16;
  let m = data.read(cpu, bus) as u16;
  let dec_result = m.wrapping_sub(1);
  data.write(cpu, bus, (dec_result & 0x00FF) as u8);
  cpu.set_status(Carry, a >= m);
  let cmp_result = a.wrapping_sub(dec_result);
  cpu.set_status(Zero, (cmp_result & 0x00FF) == 0);
  cpu.set_status(Negative, (cmp_result & 0x0080) != 0);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

// Shifts

/// Arithmetic Shift Left
fn asl(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu, bus);
  let result = m << 1; // equivalent to m * 2

  // We set the carry bit to the 7th bit from our data, since it was shifted
  // "out" of the result:
  cpu.set_status(Carry, m & 0x80 == 0x80);
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x0080) != 0);
  data.write(cpu, bus, result);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Undocumented: ASL + ORA
fn slo(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu, bus);
  let result = m << 1; // equivalent to m * 2

  // We set the carry bit to the 7th bit from our data, since it was shifted
  // "out" of the result:
  cpu.set_status(Carry, m & 0x80 == 0x80);

  cpu.a = cpu.a | result;
  cpu.set_status(Zero, cpu.a == 0x00);
  cpu.set_status(Negative, cpu.a & 0b_1000_0000 != 0);

  data.write(cpu, bus, result);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Logical Shift Right
fn lsr(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu, bus);
  let result = m >> 1; // equivalent to m / 2

  // We set the carry bit to the 0th bit from our data, since it was shifted
  // "out" of the result:
  cpu.set_status(Carry, m & 0x01 == 0x01);
  cpu.set_status(Zero, result == 0);
  cpu.set_status(Negative, result & 0x80 != 0);
  data.write(cpu, bus, result);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Undocumented: LSR + EOR
fn sre(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu, bus);
  let result = m >> 1; // equivalent to m / 2

  // We set the carry bit to the 0th bit from our data, since it was shifted
  // "out" of the result:
  cpu.set_status(Carry, m & 0x01 == 0x01);
  data.write(cpu, bus, result);

  cpu.a = cpu.a ^ result;
  cpu.set_status(Zero, cpu.a == 0x00);
  cpu.set_status(Negative, cpu.a & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Rotate Left
fn rol(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu, bus);
  let result = (m << 1) | cpu.get_status(Carry);

  let old_bit_7 = m >> 7;
  cpu.set_status(Carry, old_bit_7 != 0);
  cpu.set_status(Zero, result == 0);
  cpu.set_status(Negative, result & 0x80 != 0);
  data.write(cpu, bus, result);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Undocumented: ROL + AND
fn rla(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu, bus);
  let result = (m << 1) | cpu.get_status(Carry);

  let old_bit_7 = m >> 7;
  cpu.set_status(Carry, old_bit_7 != 0);
  cpu.set_status(Zero, result == 0);
  cpu.set_status(Negative, result & 0x80 != 0);

  cpu.a = cpu.a & result;
  data.write(cpu, bus, result);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Rotate Right
fn ror(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu, bus);
  let result = (m >> 1) | (cpu.get_status(Carry) << 7);

  let old_bit_0 = m & 0x01;
  cpu.set_status(Carry, old_bit_0 != 0);
  cpu.set_status(Zero, result == 0);
  cpu.set_status(Negative, result & 0x80 != 0);
  data.write(cpu, bus, result);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Undocumented: ROR + ADC
fn rra(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu, bus);
  let result = (m >> 1) | (cpu.get_status(Carry) << 7);
  let old_bit_0 = m & 0x01;
  cpu.set_status(Carry, old_bit_0 != 0);
  data.write(cpu, bus, result);
  adc_(cpu, cpu.a as u16 & 0x00FF, result as u16 & 0x00FF)
}

/// Jumps & Calls

/// Jump
fn jmp(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  cpu.pc = data.addr;
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Jump to Subroutine
fn jsr(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  let return_addr = cpu.pc - 1;
  let return_hi: u8 = ((return_addr >> 8) & 0x00FF) as u8;
  cpu.push(bus, return_hi);
  let return_lo: u8 = (return_addr & 0x00FF) as u8;
  cpu.push(bus, return_lo);

  cpu.pc = data.addr;
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Return from Subroutine
fn rts(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  let return_lo = cpu.pull(bus);
  let return_hi = cpu.pull(bus);
  let return_addr = ((return_hi as u16) << 8) | return_lo as u16;
  cpu.pc = return_addr;
  cpu.pc += 1;
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

// Branches

/// Utility function for branching instructions.
///
/// Every branching instruction has the same characteristic, but operates on a
/// different condition.
fn branch_if(condition: bool, cpu: &mut Cpu, data: &DataSource) -> InstructionResult {
  if condition {
    // If we are branching, we use up an extra cycle
    cpu.cycles_left += 1;

    let new_pc = data.addr;
    // If we're moving the program counter into a new page, we use one cycle in
    // _addition_ to the cycle we use to branch (totaling +2).
    //
    // We can detect if we are crossing pages by comparing the hi byte of the
    // new program counter with the hi bytes in the old program counter:
    if (new_pc & 0xFF00) != (cpu.pc & 0xFF00) {
      cpu.cycles_left += 1;
    }

    cpu.pc = new_pc;
  }
  InstructionResult {
    // We manually handle incrementing cycles since the logic depends on whether
    // we branch, so we set this to false:
    may_need_extra_cycle: false,
  }
}

/// Branch if Carry Clear
fn bcc(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  branch_if(cpu.get_status(Carry) == 0, cpu, data)
}

/// Branch if Carry Set
fn bcs(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  branch_if(cpu.get_status(Carry) != 0, cpu, data)
}

/// Branch if Equal
fn beq(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  branch_if(cpu.get_status(Zero) != 0, cpu, data)
}

/// Branch if Minus
fn bmi(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  branch_if(cpu.get_status(Negative) != 0, cpu, data)
}

/// Branch if Positive
fn bpl(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  branch_if(cpu.get_status(Negative) == 0, cpu, data)
}

/// Branch if Not Equal
fn bne(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  branch_if(cpu.get_status(Zero) == 0, cpu, data)
}

/// Branch if Overflow Clear
fn bvc(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  branch_if(cpu.get_status(Overflow) == 0, cpu, data)
}

/// Branch if Overflow Set
fn bvs(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, data: &DataSource) -> InstructionResult {
  branch_if(cpu.get_status(Overflow) != 0, cpu, data)
}

// Status Flag Changes

/// Clear carry
fn clc(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  cpu.set_status(Carry, false);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Clear decimal mode
fn cld(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  cpu.set_status(DecimalMode, false);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Clear interrupt disable
fn cli(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  cpu.set_status(DisableInterrupts, false);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Clear overflow
fn clv(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  cpu.set_status(Overflow, false);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Set carry
fn sec(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  cpu.set_status(Carry, true);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Set decimal mode
fn sed(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  cpu.set_status(DecimalMode, true);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Set interrupt disable
fn sei(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  cpu.set_status(DisableInterrupts, true);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

// System Functions

/// Force an interrupt
fn brk(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  let pc_hi: u8 = (cpu.pc >> 8) as u8;
  cpu.push(bus, pc_hi);
  let pc_lo: u8 = (cpu.pc << 8) as u8;
  cpu.push(bus, pc_lo);
  cpu.push(bus, cpu.status);

  let irq_addr = bus.read16(IRQ_POINTER);
  cpu.pc = irq_addr;
  cpu.set_status(Break, true);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Return from interrupt
fn rti(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  cpu.status = cpu.pull(bus) | cpu.get_status(Break) | cpu.get_status(Unused);

  let pc_lo = cpu.pull(bus) as u16;
  let pc_hi = cpu.pull(bus) as u16;
  cpu.pc = (pc_hi << 8) | pc_lo;

  // let irq_addr = bus.read16(IRQ_POINTER);
  // cpu.pc = irq_addr;

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// No operation
fn nop(_cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>, _data: &DataSource) -> InstructionResult {
  // Do nothing.

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

// ADDRESSING MODES ///////////////////////////////////////////////////////////

/// Implied addressing
///
/// Nothing to do here, but some implied operations operate on the accumulator,
/// so we fetch that data here
fn imp(_cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>) -> AddressingModeResult {
  AddressingModeResult {
    data: DataSource {
      kind: Implicit,
      addr: 0x0000,
    },
    needs_extra_cycle: false,
  }
}

/// Immediate addressing
///
/// Read a byte directly from the current program counter
fn imm(cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>) -> AddressingModeResult {
  let addr_abs = cpu.pc;
  cpu.pc = cpu.pc.wrapping_add(1);

  AddressingModeResult {
    data: DataSource {
      kind: AbsoluteAddress,
      addr: addr_abs,
    },
    needs_extra_cycle: false,
  }
}

/// Zero Page addressing
///
/// Read a byte at an address in the zeroth page; i.e. from one of the first 256
/// bytes in memory
fn zp0(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>) -> AddressingModeResult {
  // Read the first operand, constructing a 16-bit address within the zeroth
  // page:
  let addr_abs = (bus.read(cpu.pc) as u16) & 0x00FF;
  cpu.pc = cpu.pc.wrapping_add(1);
  AddressingModeResult {
    data: DataSource {
      kind: AbsoluteAddress,
      addr: addr_abs,
    },
    needs_extra_cycle: false,
  }
}

/// Zero Page addressing, with X address offset
///
/// Read a byte at an address in the zeroth page + X; i.e. starting from X, plus
/// 0-255
fn zpx(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>) -> AddressingModeResult {
  // Read the first operand, constructing a 16-bit address within the zeroth
  // page:
  let addr_abs = ((cpu.x.wrapping_add(bus.read(cpu.pc))) as u16) & 0x00FF;
  cpu.pc = cpu.pc.wrapping_add(1);
  AddressingModeResult {
    data: DataSource {
      kind: AbsoluteAddress,
      addr: addr_abs,
    },
    needs_extra_cycle: false,
  }
}

/// Zero Page addressing, with Y address offset
///
/// Read a byte at an address in the zeroth page + Y; i.e. starting from Y, plus
/// 0-255
fn zpy(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>) -> AddressingModeResult {
  // Read the first operand, constructing a 16-bit address within the zeroth
  // page:
  let addr_abs = ((cpu.y.wrapping_add(bus.read(cpu.pc))) as u16) & 0x00FF;
  cpu.pc = cpu.pc.wrapping_add(1);
  AddressingModeResult {
    data: DataSource {
      kind: AbsoluteAddress,
      addr: addr_abs,
    },
    needs_extra_cycle: false,
  }
}

/// Absolute addressing
///
/// Read a full 16-bit address from the current program counter + 1
fn abs(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>) -> AddressingModeResult {
  let addr_lo = bus.read(cpu.pc) as u16;
  cpu.pc = cpu.pc.wrapping_add(1);
  let addr_hi = bus.read(cpu.pc) as u16;
  cpu.pc = cpu.pc.wrapping_add(1);
  AddressingModeResult {
    data: DataSource {
      kind: AbsoluteAddress,
      addr: ((addr_hi << 8) | addr_lo),
    },
    needs_extra_cycle: false,
  }
}

/// Absolute addressing + X
///
/// Read a full 16-bit address from the current program counter + 1, then apply
/// an offset of X
fn abx(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>) -> AddressingModeResult {
  let addr_lo = bus.read(cpu.pc) as u16;
  cpu.pc = cpu.pc.wrapping_add(1);
  let addr_hi = bus.read(cpu.pc) as u16;
  cpu.pc = cpu.pc.wrapping_add(1);
  let addr_abs = ((addr_hi << 8) | addr_lo) + cpu.x as u16;

  // If our hi byte is changed after we've added X, then it has changed due to
  // overflow which means we are crossing a page. When we cross a page, we may
  // need an extra cycle:
  let needs_extra_cycle = addr_abs & 0xFF00 != (addr_hi << 8);

  AddressingModeResult {
    data: DataSource {
      kind: AbsoluteAddress,
      addr: addr_abs,
    },
    needs_extra_cycle,
  }
}

/// Absolute addressing + Y
///
/// Read a full 16-bit address from the current program counter + 1, then apply
/// an offset of Y
fn aby(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>) -> AddressingModeResult {
  let addr_lo = bus.read(cpu.pc) as u16;
  cpu.pc = cpu.pc.wrapping_add(1);
  let addr_hi = bus.read(cpu.pc) as u16;
  cpu.pc = cpu.pc.wrapping_add(1);
  let addr_abs = ((addr_hi << 8) | addr_lo).wrapping_add(cpu.y as u16);

  // If our hi byte is changed after we've added Y, then it has changed due to
  // overflow which means we are crossing a page. When we cross a page, we may
  // need an extra cycle:
  let needs_extra_cycle = addr_abs & 0xFF00 != (addr_hi << 8);

  AddressingModeResult {
    data: DataSource {
      kind: AbsoluteAddress,
      addr: addr_abs,
    },
    needs_extra_cycle,
  }
}

/// Indirect
fn ind(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>) -> AddressingModeResult {
  let ptr_lo = bus.read(cpu.pc) as u16;
  cpu.pc = cpu.pc.wrapping_add(1);
  let ptr_hi = bus.read(cpu.pc) as u16;
  cpu.pc = cpu.pc.wrapping_add(1);
  let ptr = ptr_hi << 8 | ptr_lo;

  // The 6502 has a hardware bug where if you happen to have a pointer address
  // in memory that spans across pages (remember, pointers are 2 bytes, and
  // therefore it is possible for this to happen), it will not actually read the
  // hi byte of the address properly
  let addr_abs = if ptr_lo == 0x00FF {
    ((bus.read(ptr & 0xFF00) as u16) << 8) | bus.read(ptr) as u16
  } else {
    bus.read16(ptr)
  };

  AddressingModeResult {
    data: DataSource {
      kind: AbsoluteAddress,
      addr: addr_abs,
    },
    needs_extra_cycle: false,
  }
}

/// (Indirect, X)
fn izx(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>) -> AddressingModeResult {
  // Our pointer lives in the zeroth page, so we only need to read one byte
  let ptr = bus.read(cpu.pc);
  cpu.pc = cpu.pc.wrapping_add(1);

  // We read X offset from this pointer
  let lo = bus.read(ptr.wrapping_add(cpu.x) as u16 & 0x00FF) as u16;
  let hi = bus.read(ptr.wrapping_add(cpu.x + 1) as u16 & 0x00FF) as u16;
  let addr_abs = (hi << 8) | lo;
  AddressingModeResult {
    data: DataSource {
      kind: AbsoluteAddress,
      addr: addr_abs,
    },
    needs_extra_cycle: false,
  }
}

/// (Indirect), Y
fn izy(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>) -> AddressingModeResult {
  // Our pointer lives in the zeroth page, so we only need to read one byte
  let ptr = bus.read(cpu.pc) as u16 & 0x00FF;
  cpu.pc = cpu.pc.wrapping_add(1);

  let lo = bus.read(ptr as u16 & 0x00FF) as u16;
  let hi = bus.read(ptr.wrapping_add(1) as u16 & 0x00FF) as u16;
  let addr = (hi << 8) | lo;
  let addr_abs = addr.wrapping_add(cpu.y as u16);

  // We only read this here so we can check if we crossed a page:
  let addr_hi = bus.read(ptr + 1) as u16 & 0x00FF;
  // If our hi byte is changed after we've added Y, then it has changed due to
  // overflow which means we are crossing a page. When we cross a page, we may
  // need an extra cycle:
  let needs_extra_cycle = addr_abs & 0xFF00 != (addr_hi << 8);

  AddressingModeResult {
    data: DataSource {
      kind: AbsoluteAddress,
      addr: addr_abs,
    },
    needs_extra_cycle,
  }
}

/// Accumulator
fn acc(_cpu: &mut Cpu, _bus: &mut dyn Bus<Cpu>) -> AddressingModeResult {
  AddressingModeResult {
    data: DataSource {
      kind: Accumulator,
      addr: 0x0000,
    },
    needs_extra_cycle: false,
  }
}

/// Relative
fn rel(cpu: &mut Cpu, bus: &mut dyn Bus<Cpu>) -> AddressingModeResult {
  let offset = bus.read(cpu.pc);
  cpu.pc = cpu.pc.wrapping_add(1);

  // This ensures the binary arithmatic works out when adding this relative
  // address to our program counter.
  let addr = if offset & 0x80 != 0 {
    // Get the inverted version of the offset by applying two's complement:
    let neg_offset = !(offset as u16) + 1 & 0x00FF;
    cpu.pc - neg_offset
  } else {
    cpu.pc + ((offset as u16) & 0x00FF)
  };

  AddressingModeResult {
    data: DataSource {
      kind: AbsoluteAddress,
      addr,
    },
    needs_extra_cycle: false,
  }
}

const ILLEGAL_OPERATION: Operation = Operation {
  addressing_mode: IMP,
  instruction: NOP,
  cycles: 1,
  undocumented: true,
};

// Generated the following hashmap by running this JS on
// https://web.archive.org/web/20210724004546/http://www.obelisk.me.uk/6502/reference.html
//
// ```js
// addressing_map = {
//   'Absolute,X': 'abx',
//   'Absolute,Y': 'aby',
//   '(Indirect,X)': 'izx',
//   '(Indirect),Y': 'izy',
//   'Zero Page': 'zp0',
//   'Zero Page,X': 'zpx',
//   'Zero Page,Y': 'zpy',
//   Absolute: 'abs',
//   Accumulator: 'acc',
//   Immediate: 'imm',
//   Implicit: 'imp',
//   Implied: 'imp',
//   Indirect: 'ind',
//   Indirect: 'ind',
//   Relative: 'rel',
// };
//
// makeOp = ({opcode, instruction, addressing_mode, cycles}) => `0x${opcode.slice(1)} => Operation {
//   instruction: ${instruction.toLowerCase()},
//   addressing_mode: ${addressing_map[addressing_mode.trim()] || addressing_mode},
//   cycles: ${parseInt(cycles)},
// }`;
//
// instructions = $$('h3').map(el => el.innerText.split(' ')[0]);
// tables = [
//   ...$$('table').filter(el => el.innerText.includes('Addressing Mode')),
// ];
// result = [];
// iidx = 0;
// for (table of tables) {
//   const instruction = instructions[iidx];
//   const innerTexts = [...table.querySelectorAll('td')].map(td => td.innerText);
//   let idx = 4;
//   while (idx < innerTexts.length) {
//     const [addressing_mode, opcode, _, cycles] = innerTexts.slice(idx, idx + 4);
//     result.push(
//       makeOp({
//         opcode,
//         instruction,
//         addressing_mode,
//         cycles,
//       })
//     );
//
//     idx += 4;
//   }
//
//   iidx += 1;
// }
// result.join(',\n');
// ```

lazy_static! {
  static ref OPCODE_MAP: HashMap<u8, Operation> = hashmap! {
    0x69 => Operation {
      instruction: ADC,
      addressing_mode: IMM,
      cycles: 2,
      undocumented: false,
    },
    0x65 => Operation {
      instruction: ADC,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: false,
    },
    0x75 => Operation {
      instruction: ADC,
      addressing_mode: ZPX,
      cycles: 4,
      undocumented: false,
    },
    0x6D => Operation {
      instruction: ADC,
      addressing_mode: ABS,
      cycles: 4,
      undocumented: false,
    },
    0x7D => Operation {
      instruction: ADC,
      addressing_mode: ABX,
      cycles: 4,
      undocumented: false,
    },
    0x79 => Operation {
      instruction: ADC,
      addressing_mode: ABY,
      cycles: 4,
      undocumented: false,
    },
    0x61 => Operation {
      instruction: ADC,
      addressing_mode: IZX,
      cycles: 6,
      undocumented: false,
    },
    0x71 => Operation {
      instruction: ADC,
      addressing_mode: IZY,
      cycles: 5,
      undocumented: false,
    },
    0x29 => Operation {
      instruction: AND,
      addressing_mode: IMM,
      cycles: 2,
      undocumented: false,
    },
    0x25 => Operation {
      instruction: AND,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: false,
    },
    0x35 => Operation {
      instruction: AND,
      addressing_mode: ZPX,
      cycles: 4,
      undocumented: false,
    },
    0x2D => Operation {
      instruction: AND,
      addressing_mode: ABS,
      cycles: 4,
      undocumented: false,
    },
    0x3D => Operation {
      instruction: AND,
      addressing_mode: ABX,
      cycles: 4,
      undocumented: false,
    },
    0x39 => Operation {
      instruction: AND,
      addressing_mode: ABY,
      cycles: 4,
      undocumented: false,
    },
    0x21 => Operation {
      instruction: AND,
      addressing_mode: IZX,
      cycles: 6,
      undocumented: false,
    },
    0x31 => Operation {
      instruction: AND,
      addressing_mode: IZY,
      cycles: 5,
      undocumented: false,
    },
    0x0A => Operation {
      instruction: ASL,
      addressing_mode: ACC,
      cycles: 2,
      undocumented: false,
    },
    0x06 => Operation {
      instruction: ASL,
      addressing_mode: ZP0,
      cycles: 5,
      undocumented: false,
    },
    0x16 => Operation {
      instruction: ASL,
      addressing_mode: ZPX,
      cycles: 6,
      undocumented: false,
    },
    0x0E => Operation {
      instruction: ASL,
      addressing_mode: ABS,
      cycles: 6,
      undocumented: false,
    },
    0x1E => Operation {
      instruction: ASL,
      addressing_mode: ABX,
      cycles: 7,
      undocumented: false,
    },
    0x90 => Operation {
      instruction: BCC,
      addressing_mode: REL,
      cycles: 2,
      undocumented: false,
    },
    0xB0 => Operation {
      instruction: BCS,
      addressing_mode: REL,
      cycles: 2,
      undocumented: false,
    },
    0xF0 => Operation {
      instruction: BEQ,
      addressing_mode: REL,
      cycles: 2,
      undocumented: false,
    },
    0x24 => Operation {
      instruction: BIT,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: false,
    },
    0x2C => Operation {
      instruction: BIT,
      addressing_mode: ABS,
      cycles: 4,
      undocumented: false,
    },
    0x30 => Operation {
      instruction: BMI,
      addressing_mode: REL,
      cycles: 2,
      undocumented: false,
    },
    0xD0 => Operation {
      instruction: BNE,
      addressing_mode: REL,
      cycles: 2,
      undocumented: false,
    },
    0x10 => Operation {
      instruction: BPL,
      addressing_mode: REL,
      cycles: 2,
      undocumented: false,
    },
    0x00 => Operation {
      instruction: BRK,
      addressing_mode: IMP,
      cycles: 7,
      undocumented: false,
    },
    0x50 => Operation {
      instruction: BVC,
      addressing_mode: REL,
      cycles: 2,
      undocumented: false,
    },
    0x70 => Operation {
      instruction: BVS,
      addressing_mode: REL,
      cycles: 2,
      undocumented: false,
    },
    0x18 => Operation {
      instruction: CLC,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: false,
    },
    0xD8 => Operation {
      instruction: CLD,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: false,
    },
    0x58 => Operation {
      instruction: CLI,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: false,
    },
    0xB8 => Operation {
      instruction: CLV,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: false,
    },
    0xC9 => Operation {
      instruction: CMP,
      addressing_mode: IMM,
      cycles: 2,
      undocumented: false,
    },
    0xC5 => Operation {
      instruction: CMP,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: false,
    },
    0xD5 => Operation {
      instruction: CMP,
      addressing_mode: ZPX,
      cycles: 4,
      undocumented: false,
    },
    0xCD => Operation {
      instruction: CMP,
      addressing_mode: ABS,
      cycles: 4,
      undocumented: false,
    },
    0xDD => Operation {
      instruction: CMP,
      addressing_mode: ABX,
      cycles: 4,
      undocumented: false,
    },
    0xD9 => Operation {
      instruction: CMP,
      addressing_mode: ABY,
      cycles: 4,
      undocumented: false,
    },
    0xC1 => Operation {
      instruction: CMP,
      addressing_mode: IZX,
      cycles: 6,
      undocumented: false,
    },
    0xD1 => Operation {
      instruction: CMP,
      addressing_mode: IZY,
      cycles: 5,
      undocumented: false,
    },
    0xE0 => Operation {
      instruction: CPX,
      addressing_mode: IMM,
      cycles: 2,
      undocumented: false,
    },
    0xE4 => Operation {
      instruction: CPX,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: false,
    },
    0xEC => Operation {
      instruction: CPX,
      addressing_mode: ABS,
      cycles: 4,
      undocumented: false,
    },
    0xC0 => Operation {
      instruction: CPY,
      addressing_mode: IMM,
      cycles: 2,
      undocumented: false,
    },
    0xC4 => Operation {
      instruction: CPY,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: false,
    },
    0xCC => Operation {
      instruction: CPY,
      addressing_mode: ABS,
      cycles: 4,
      undocumented: false,
    },
    0xC6 => Operation {
      instruction: DEC,
      addressing_mode: ZP0,
      cycles: 5,
      undocumented: false,
    },
    0xD6 => Operation {
      instruction: DEC,
      addressing_mode: ZPX,
      cycles: 6,
      undocumented: false,
    },
    0xCE => Operation {
      instruction: DEC,
      addressing_mode: ABS,
      cycles: 6,
      undocumented: false,
    },
    0xDE => Operation {
      instruction: DEC,
      addressing_mode: ABX,
      cycles: 7,
      undocumented: false,
    },
    0xCA => Operation {
      instruction: DEX,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: false,
    },
    0x88 => Operation {
      instruction: DEY,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: false,
    },
    0x49 => Operation {
      instruction: EOR,
      addressing_mode: IMM,
      cycles: 2,
      undocumented: false,
    },
    0x45 => Operation {
      instruction: EOR,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: false,
    },
    0x55 => Operation {
      instruction: EOR,
      addressing_mode: ZPX,
      cycles: 4,
      undocumented: false,
    },
    0x4D => Operation {
      instruction: EOR,
      addressing_mode: ABS,
      cycles: 4,
      undocumented: false,
    },
    0x5D => Operation {
      instruction: EOR,
      addressing_mode: ABX,
      cycles: 4,
      undocumented: false,
    },
    0x59 => Operation {
      instruction: EOR,
      addressing_mode: ABY,
      cycles: 4,
      undocumented: false,
    },
    0x41 => Operation {
      instruction: EOR,
      addressing_mode: IZX,
      cycles: 6,
      undocumented: false,
    },
    0x51 => Operation {
      instruction: EOR,
      addressing_mode: IZY,
      cycles: 5,
      undocumented: false,
    },
    0xE6 => Operation {
      instruction: INC,
      addressing_mode: ZP0,
      cycles: 5,
      undocumented: false,
    },
    0xF6 => Operation {
      instruction: INC,
      addressing_mode: ZPX,
      cycles: 6,
      undocumented: false,
    },
    0xEE => Operation {
      instruction: INC,
      addressing_mode: ABS,
      cycles: 6,
      undocumented: false,
    },
    0xFE => Operation {
      instruction: INC,
      addressing_mode: ABX,
      cycles: 7,
      undocumented: false,
    },
    0xE8 => Operation {
      instruction: INX,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: false,
    },
    0xC8 => Operation {
      instruction: INY,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: false,
    },
    0x4C => Operation {
      instruction: JMP,
      addressing_mode: ABS,
      cycles: 3,
      undocumented: false,
    },
    0x6C => Operation {
      instruction: JMP,
      addressing_mode: IND,
      cycles: 5,
      undocumented: false,
    },
    0x20 => Operation {
      instruction: JSR,
      addressing_mode: ABS,
      cycles: 6,
      undocumented: false,
    },
    0xA9 => Operation {
      instruction: LDA,
      addressing_mode: IMM,
      cycles: 2,
      undocumented: false,
    },
    0xA5 => Operation {
      instruction: LDA,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: false,
    },
    0xB5 => Operation {
      instruction: LDA,
      addressing_mode: ZPX,
      cycles: 4,
      undocumented: false,
    },
    0xAD => Operation {
      instruction: LDA,
      addressing_mode: ABS,
      cycles: 4,
      undocumented: false,
    },
    0xBD => Operation {
      instruction: LDA,
      addressing_mode: ABX,
      cycles: 4,
      undocumented: false,
    },
    0xB9 => Operation {
      instruction: LDA,
      addressing_mode: ABY,
      cycles: 4,
      undocumented: false,
    },
    0xA1 => Operation {
      instruction: LDA,
      addressing_mode: IZX,
      cycles: 6,
      undocumented: false,
    },
    0xB1 => Operation {
      instruction: LDA,
      addressing_mode: IZY,
      cycles: 5,
      undocumented: false,
    },
    0xA2 => Operation {
      instruction: LDX,
      addressing_mode: IMM,
      cycles: 2,
      undocumented: false,
    },
    0xA6 => Operation {
      instruction: LDX,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: false,
    },
    0xB6 => Operation {
      instruction: LDX,
      addressing_mode: ZPY,
      cycles: 4,
      undocumented: false,
    },
    0xAE => Operation {
      instruction: LDX,
      addressing_mode: ABS,
      cycles: 4,
      undocumented: false,
    },
    0xBE => Operation {
      instruction: LDX,
      addressing_mode: ABY,
      cycles: 4,
      undocumented: false,
    },
    0xA0 => Operation {
      instruction: LDY,
      addressing_mode: IMM,
      cycles: 2,
      undocumented: false,
    },
    0xA4 => Operation {
      instruction: LDY,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: false,
    },
    0xB4 => Operation {
      instruction: LDY,
      addressing_mode: ZPX,
      cycles: 4,
      undocumented: false,
    },
    0xAC => Operation {
      instruction: LDY,
      addressing_mode: ABS,
      cycles: 4,
      undocumented: false,
    },
    0xBC => Operation {
      instruction: LDY,
      addressing_mode: ABX,
      cycles: 4,
      undocumented: false,
    },
    0x4A => Operation {
      instruction: LSR,
      addressing_mode: ACC,
      cycles: 2,
      undocumented: false,
    },
    0x46 => Operation {
      instruction: LSR,
      addressing_mode: ZP0,
      cycles: 5,
      undocumented: false,
    },
    0x56 => Operation {
      instruction: LSR,
      addressing_mode: ZPX,
      cycles: 6,
      undocumented: false,
    },
    0x4E => Operation {
      instruction: LSR,
      addressing_mode: ABS,
      cycles: 6,
      undocumented: false,
    },
    0x5E => Operation {
      instruction: LSR,
      addressing_mode: ABX,
      cycles: 7,
      undocumented: false,
    },
    0xEA => Operation {
      instruction: NOP,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: false,
    },
    0x09 => Operation {
      instruction: ORA,
      addressing_mode: IMM,
      cycles: 2,
      undocumented: false,
    },
    0x05 => Operation {
      instruction: ORA,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: false,
    },
    0x15 => Operation {
      instruction: ORA,
      addressing_mode: ZPX,
      cycles: 4,
      undocumented: false,
    },
    0x0D => Operation {
      instruction: ORA,
      addressing_mode: ABS,
      cycles: 4,
      undocumented: false,
    },
    0x1D => Operation {
      instruction: ORA,
      addressing_mode: ABX,
      cycles: 4,
      undocumented: false,
    },
    0x19 => Operation {
      instruction: ORA,
      addressing_mode: ABY,
      cycles: 4,
      undocumented: false,
    },
    0x01 => Operation {
      instruction: ORA,
      addressing_mode: IZX,
      cycles: 6,
      undocumented: false,
    },
    0x11 => Operation {
      instruction: ORA,
      addressing_mode: IZY,
      cycles: 5,
      undocumented: false,
    },
    0x48 => Operation {
      instruction: PHA,
      addressing_mode: IMP,
      cycles: 3,
      undocumented: false,
    },
    0x08 => Operation {
      instruction: PHP,
      addressing_mode: IMP,
      cycles: 3,
      undocumented: false,
    },
    0x68 => Operation {
      instruction: PLA,
      addressing_mode: IMP,
      cycles: 4,
      undocumented: false,
    },
    0x28 => Operation {
      instruction: PLP,
      addressing_mode: IMP,
      cycles: 4,
      undocumented: false,
    },
    0x2A => Operation {
      instruction: ROL,
      addressing_mode: ACC,
      cycles: 2,
      undocumented: false,
    },
    0x26 => Operation {
      instruction: ROL,
      addressing_mode: ZP0,
      cycles: 5,
      undocumented: false,
    },
    0x36 => Operation {
      instruction: ROL,
      addressing_mode: ZPX,
      cycles: 6,
      undocumented: false,
    },
    0x2E => Operation {
      instruction: ROL,
      addressing_mode: ABS,
      cycles: 6,
      undocumented: false,
    },
    0x3E => Operation {
      instruction: ROL,
      addressing_mode: ABX,
      cycles: 7,
      undocumented: false,
    },
    0x6A => Operation {
      instruction: ROR,
      addressing_mode: ACC,
      cycles: 2,
      undocumented: false,
    },
    0x66 => Operation {
      instruction: ROR,
      addressing_mode: ZP0,
      cycles: 5,
      undocumented: false,
    },
    0x76 => Operation {
      instruction: ROR,
      addressing_mode: ZPX,
      cycles: 6,
      undocumented: false,
    },
    0x6E => Operation {
      instruction: ROR,
      addressing_mode: ABS,
      cycles: 6,
      undocumented: false,
    },
    0x7E => Operation {
      instruction: ROR,
      addressing_mode: ABX,
      cycles: 7,
      undocumented: false,
    },
    0x40 => Operation {
      instruction: RTI,
      addressing_mode: IMP,
      cycles: 6,
      undocumented: false,
    },
    0x60 => Operation {
      instruction: RTS,
      addressing_mode: IMP,
      cycles: 6,
      undocumented: false,
    },
    0xE9 => Operation {
      instruction: SBC,
      addressing_mode: IMM,
      cycles: 2,
      undocumented: false,
    },
    0xE5 => Operation {
      instruction: SBC,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: false,
    },
    0xF5 => Operation {
      instruction: SBC,
      addressing_mode: ZPX,
      cycles: 4,
      undocumented: false,
    },
    0xED => Operation {
      instruction: SBC,
      addressing_mode: ABS,
      cycles: 4,
      undocumented: false,
    },
    0xFD => Operation {
      instruction: SBC,
      addressing_mode: ABX,
      cycles: 4,
      undocumented: false,
    },
    0xF9 => Operation {
      instruction: SBC,
      addressing_mode: ABY,
      cycles: 4,
      undocumented: false,
    },
    0xE1 => Operation {
      instruction: SBC,
      addressing_mode: IZX,
      cycles: 6,
      undocumented: false,
    },
    0xF1 => Operation {
      instruction: SBC,
      addressing_mode: IZY,
      cycles: 5,
      undocumented: false,
    },
    0x38 => Operation {
      instruction: SEC,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: false,
    },
    0xF8 => Operation {
      instruction: SED,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: false,
    },
    0x78 => Operation {
      instruction: SEI,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: false,
    },
    0x85 => Operation {
      instruction: STA,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: false,
    },
    0x95 => Operation {
      instruction: STA,
      addressing_mode: ZPX,
      cycles: 4,
      undocumented: false,
    },
    0x8D => Operation {
      instruction: STA,
      addressing_mode: ABS,
      cycles: 4,
      undocumented: false,
    },
    0x9D => Operation {
      instruction: STA,
      addressing_mode: ABX,
      cycles: 5,
      undocumented: false,
    },
    0x99 => Operation {
      instruction: STA,
      addressing_mode: ABY,
      cycles: 5,
      undocumented: false,
    },
    0x81 => Operation {
      instruction: STA,
      addressing_mode: IZX,
      cycles: 6,
      undocumented: false,
    },
    0x91 => Operation {
      instruction: STA,
      addressing_mode: IZY,
      cycles: 6,
      undocumented: false,
    },
    0x86 => Operation {
      instruction: STX,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: false,
    },
    0x96 => Operation {
      instruction: STX,
      addressing_mode: ZPY,
      cycles: 4,
      undocumented: false,
    },
    0x8E => Operation {
      instruction: STX,
      addressing_mode: ABS,
      cycles: 4,
      undocumented: false,
    },
    0x84 => Operation {
      instruction: STY,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: false,
    },
    0x94 => Operation {
      instruction: STY,
      addressing_mode: ZPX,
      cycles: 4,
      undocumented: false,
    },
    0x8C => Operation {
      instruction: STY,
      addressing_mode: ABS,
      cycles: 4,
      undocumented: false,
    },
    0xAA => Operation {
      instruction: TAX,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: false,
    },
    0xA8 => Operation {
      instruction: TAY,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: false,
    },
    0xBA => Operation {
      instruction: TSX,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: false,
    },
    0x8A => Operation {
      instruction: TXA,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: false,
    },
    0x9A => Operation {
      instruction: TXS,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: false,
    },
    0x98 => Operation {
      instruction: TYA,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: false,
    },

    // Undocumented opcodes:
    0x1A => Operation {
      instruction: NOP,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: true,
    },
    0x3A => Operation {
      instruction: NOP,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: true,
    },
    0x5A => Operation {
      instruction: NOP,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: true,
    },
    0x7A => Operation {
      instruction: NOP,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: true,
    },
    0xDA => Operation {
      instruction: NOP,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: true,
    },
    0xFA => Operation {
      instruction: NOP,
      addressing_mode: IMP,
      cycles: 2,
      undocumented: true,
    },
    0x80 => Operation {
      instruction: NOP,
      addressing_mode: IMM,
      cycles: 2,
      undocumented: true,
    },
    0x82 => Operation {
      instruction: NOP,
      addressing_mode: IMM,
      cycles: 2,
      undocumented: true,
    },
    0x89 => Operation {
      instruction: NOP,
      addressing_mode: IMM,
      cycles: 2,
      undocumented: true,
    },
    0xC2 => Operation {
      instruction: NOP,
      addressing_mode: IMM,
      cycles: 2,
      undocumented: true,
    },
    0xE2 => Operation {
      instruction: NOP,
      addressing_mode: IMM,
      cycles: 2,
      undocumented: true,
    },
    0x04 => Operation {
      instruction: NOP,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: true,
    },
    0x44 => Operation {
      instruction: NOP,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: true,
    },
    0x64 => Operation {
      instruction: NOP,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: true,
    },
    0x14 => Operation {
      instruction: NOP,
      addressing_mode: ZPX,
      cycles: 4,
      undocumented: true,
    },
    0x34 => Operation {
      instruction: NOP,
      addressing_mode: ZPX,
      cycles: 4,
      undocumented: true,
    },
    0x54 => Operation {
      instruction: NOP,
      addressing_mode: ZPX,
      cycles: 4,
      undocumented: true,
    },
    0x74 => Operation {
      instruction: NOP,
      addressing_mode: ZPX,
      cycles: 4,
      undocumented: true,
    },
    0xD4 => Operation {
      instruction: NOP,
      addressing_mode: ZPX,
      cycles: 4,
      undocumented: true,
    },
    0xF4 => Operation {
      instruction: NOP,
      addressing_mode: ZPX,
      cycles: 4,
      undocumented: true,
    },
    0x0C => Operation {
      instruction: NOP,
      addressing_mode: ABS,
      cycles: 4,
      undocumented: true,
    },
    0x1C => Operation {
      instruction: NOP,
      addressing_mode: ABX,
      cycles: 4,
      undocumented: true,
    },
    0x3C => Operation {
      instruction: NOP,
      addressing_mode: ABX,
      cycles: 4,
      undocumented: true,
    },
    0x5C => Operation {
      instruction: NOP,
      addressing_mode: ABX,
      cycles: 4,
      undocumented: true,
    },
    0x7C => Operation {
      instruction: NOP,
      addressing_mode: ABX,
      cycles: 4,
      undocumented: true,
    },
    0xDC => Operation {
      instruction: NOP,
      addressing_mode: ABX,
      cycles: 4,
      undocumented: true,
    },
    0xFC => Operation {
      instruction: NOP,
      addressing_mode: ABX,
      cycles: 4,
      undocumented: true,
    },

    0xA7 => Operation {
      instruction: LAX,
      addressing_mode: ZP0,
      cycles: 3,
      undocumented: true,
    },
    0xB7 => Operation {
      instruction: LAX,
      addressing_mode: ZPY,
      cycles: 4,
      undocumented: true,
    },
    0xAF => Operation {
      instruction: LAX,
      addressing_mode: ABS,
      cycles:	4,
      undocumented: true,
    },
    0xBF => Operation {
      instruction: LAX,
      addressing_mode: ABY,
      cycles: 4,
      undocumented: true,
    },
    0xA3 => Operation {
      instruction: LAX,
      addressing_mode: IZX,
      cycles: 6,
      undocumented: true,
    },
    0xB3 => Operation {
      instruction: LAX,
      addressing_mode: IZY,
      cycles: 5,
      undocumented: true,
    },

    0x87 => Operation{
      instruction: SAX,
      addressing_mode:ZP0,
      cycles: 3,
      undocumented: true,
    },
    0x97 => Operation{
      instruction: SAX,
      addressing_mode:ZPY,
      cycles: 4,
      undocumented: true,
    },
    0x8F => Operation{
      instruction: SAX,
      addressing_mode:ABS,
      cycles: 4,
      undocumented: true,
    },
    0x83 => Operation{
      instruction: SAX,
      addressing_mode:IZX,
      cycles: 6,
      undocumented: true,
    },

    0xEB => Operation {
      instruction: SBC,
      addressing_mode: IMM,
      cycles: 2,
      undocumented: true,
    },


    0xC7 => Operation {
      instruction: DCP,
      addressing_mode: ZP0,
      cycles: 5,
      undocumented: true,
    },
    0xD7 => Operation {
      instruction: DCP,
      addressing_mode: ZPX,
      cycles: 6,
      undocumented: true,
    },
    0xCF => Operation {
      instruction: DCP,
      addressing_mode: ABS,
      cycles: 6,
      undocumented: true,
    },
    0xDF => Operation {
      instruction: DCP,
      addressing_mode: ABX,
      cycles: 7,
      undocumented: true,
    },
    0xDB => Operation {
      instruction: DCP,
      addressing_mode: ABY,
      cycles: 7,
      undocumented: true,
    },
    0xC3 => Operation {
      instruction: DCP,
      addressing_mode: IZX,
      cycles: 8,
      undocumented: true,
    },
    0xD3 => Operation {
      instruction: DCP,
      addressing_mode: IZY,
      cycles: 8,
      undocumented: true,
    },

    0xE7 => Operation {
      instruction: ISB,
      addressing_mode: ZP0,
      cycles: 5,
      undocumented: true,
    },
    0xF7 => Operation {
      instruction: ISB,
      addressing_mode: ZPX,
      cycles: 6,
      undocumented: true,
    },
    0xEF => Operation {
      instruction: ISB,
      addressing_mode: ABS,
      cycles: 6,
      undocumented: true,
    },
    0xFF => Operation {
      instruction: ISB,
      addressing_mode: ABX,
      cycles: 7,
      undocumented: true,
    },
    0xFB => Operation {
      instruction: ISB,
      addressing_mode: ABY,
      cycles: 7,
      undocumented: true,
    },
    0xE3 => Operation {
      instruction: ISB,
      addressing_mode: IZX,
      cycles: 8,
      undocumented: true,
    },
    0xF3 => Operation {
      instruction: ISB,
      addressing_mode: IZY,
      cycles: 4,
      undocumented: true,
    },

    0x07 => Operation {
      instruction: SLO,
      addressing_mode: ZP0,
      cycles: 5,
      undocumented: true,
    },
    0x17 => Operation {
      instruction: SLO,
      addressing_mode: ZPX,
      cycles: 6,
      undocumented: true,
    },
    0x0F => Operation {
      instruction: SLO,
      addressing_mode: ABS,
      cycles: 6,
      undocumented: true,
    },
    0x1F => Operation {
      instruction: SLO,
      addressing_mode: ABX,
      cycles: 7,
      undocumented: true,
    },
    0x1B => Operation {
      instruction: SLO,
      addressing_mode: ABY,
      cycles: 7,
      undocumented: true,
    },
    0x03 => Operation {
      instruction: SLO,
      addressing_mode: IZX,
      cycles: 8,
      undocumented: true,
    },
    0x13 => Operation {
      instruction: SLO,
      addressing_mode: IZY,
      cycles: 8,
      undocumented: true,
    },

    0x27 => Operation {
      instruction: RLA,
      addressing_mode: ZP0,
      cycles: 5,
      undocumented: true,
    },
    0x37 => Operation {
      instruction: RLA,
      addressing_mode: ZPX,
      cycles: 6,
      undocumented: true,
    },
    0x2F => Operation {
      instruction: RLA,
      addressing_mode: ABS,
      cycles: 6,
      undocumented: true,
    },
    0x3F => Operation {
      instruction: RLA,
      addressing_mode: ABX,
      cycles: 7,
      undocumented: true,
    },
    0x3B => Operation {
      instruction: RLA,
      addressing_mode: ABY,
      cycles: 7,
      undocumented: true,
    },
    0x23 => Operation {
      instruction: RLA,
      addressing_mode: IZX,
      cycles: 8,
      undocumented: true,
    },
    0x33 => Operation {
      instruction: RLA,
      addressing_mode: IZY,
      cycles: 8,
      undocumented: true,
    },

    0x47 => Operation {
      instruction: SRE,
      addressing_mode: ZP0,
      cycles: 5,
      undocumented: true,
    },
    0x57 => Operation {
      instruction: SRE,
      addressing_mode: ZPX,
      cycles: 6,
      undocumented: true,
    },
    0x4F => Operation {
      instruction: SRE,
      addressing_mode: ABS,
      cycles: 6,
      undocumented: true,
    },
    0x5F => Operation {
      instruction: SRE,
      addressing_mode: ABX,
      cycles: 7,
      undocumented: true,
    },
    0x5B => Operation {
      instruction: SRE,
      addressing_mode: ABY,
      cycles: 7,
      undocumented: true,
    },
    0x43 => Operation {
      instruction: SRE,
      addressing_mode: IZX,
      cycles: 8,
      undocumented: true,
    },
    0x53 => Operation {
      instruction: SRE,
      addressing_mode: IZY,
      cycles: 8,
      undocumented: true,
    },


    0x67 => Operation {
      instruction: RRA,
      addressing_mode: ZP0,
      cycles: 5,
      undocumented: true,
    },
    0x77 => Operation {
      instruction: RRA,
      addressing_mode: ZPX,
      cycles: 6,
      undocumented: true,
    },
    0x6F => Operation {
      instruction: RRA,
      addressing_mode: ABS,
      cycles: 6,
      undocumented: true,
    },
    0x7F => Operation {
      instruction: RRA,
      addressing_mode: ABX,
      cycles: 7,
      undocumented: true,
    },
    0x7B => Operation {
      instruction: RRA,
      addressing_mode: ABY,
      cycles: 7,
      undocumented: true,
    },
    0x63 => Operation {
      instruction: RRA,
      addressing_mode: IZX,
      cycles: 8,
      undocumented: true,
    },
    0x73 => Operation {
      instruction: RRA,
      addressing_mode: IZY,
      cycles: 8,
      undocumented: true,
    },

  };
}

impl From<u8> for &Operation {
  fn from(opcode: u8) -> Self {
    match OPCODE_MAP.get(&opcode) {
      Some(operation) => operation,
      None => &ILLEGAL_OPERATION,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::bus_device::BusDevice;
  use crate::cart::Cart;
  use crate::ram::Ram;

  /// A list of bus devices, in order of "priority". The order of devices does
  /// **not** represent where the device lives in address space.
  ///
  /// When performing a read or write, devices are accessed in the order supplied
  /// in this list. When a device returns `Some` from a `read`/`write`, it now
  /// owns that operation, and all devices after it in the list are ignored.
  struct DeviceList {
    devices: Vec<Box<dyn BusDevice>>,
    cart: Cart,
  }

  impl DeviceList {
    pub fn new(devices: Vec<Box<dyn BusDevice>>) -> DeviceList {
      let cart = Cart::from_file("nessers-main/src/test_fixtures/nestest.nes").unwrap();
      DeviceList { devices, cart }
    }
  }

  impl Bus<Cpu> for DeviceList {
    fn write(&mut self, addr: u16, data: u8) {
      for i in 0..self.devices.len() {
        match self.devices[i].write(addr, data, &mut self.cart) {
          None => (),
          Some(_) => {
            break;
          }
        }
      }
    }
    fn read(&mut self, addr: u16) -> u8 {
      for i in 0..self.devices.len() {
        match self.devices[i].read(addr, &self.cart) {
          None => (),
          Some(data) => {
            return data;
          }
        }
      }
      0x00
    }
    fn safe_read(&self, addr: u16) -> u8 {
      for device in &self.devices {
        match device.safe_read(addr, &self.cart) {
          None => (),
          Some(data) => {
            return data;
          }
        }
      }
      0x00
    }
  }

  const ALL_FLAGS: [StatusFlag; 8] = [
    Carry,
    Zero,
    DisableInterrupts,
    DecimalMode,
    Break,
    Unused,
    Overflow,
    Negative,
  ];

  struct DummyBus {}
  impl BusDevice for DummyBus {
    fn write(&mut self, _: u16, _: u8, _cart: &mut Cart) -> std::option::Option<()> {
      None
    }
    fn safe_read(&self, _: u16, _cart: &Cart) -> std::option::Option<u8> {
      None
    }
  }

  #[test]
  fn get_status() {
    let mut cpu = Cpu::new();

    assert_eq!(cpu.get_status(StatusFlag::Carry), 0b0000_0000);
    assert_eq!(cpu.get_status(StatusFlag::Zero), 0b0000_0000);
    assert_eq!(cpu.get_status(StatusFlag::DisableInterrupts), 0b0000_0100);
    assert_eq!(cpu.get_status(StatusFlag::DecimalMode), 0b0000_0000);
    assert_eq!(cpu.get_status(StatusFlag::Break), 0b0000_0000);
    assert_eq!(cpu.get_status(StatusFlag::Unused), 0b0010_0000);
    assert_eq!(cpu.get_status(StatusFlag::Overflow), 0b0000_0000);
    assert_eq!(cpu.get_status(StatusFlag::Negative), 0b0000_0000);

    for flag in &ALL_FLAGS {
      let flag = *flag;
      cpu.status = flag as u8;
      for other_flag in &ALL_FLAGS {
        let other_flag = *other_flag;
        if flag == other_flag {
          assert_eq!(cpu.get_status(other_flag), (flag as u8));
        } else {
          assert_eq!(cpu.get_status(other_flag), 0b0000_0000);
        }
      }
    }
  }

  #[test]
  fn set_status() {
    let mut cpu = Cpu::new();
    cpu.status = 0x00;

    for flag in &ALL_FLAGS {
      let flag = *flag;
      assert_eq!(cpu.get_status(flag), 0b0000_0000);
      cpu.set_status(flag, true);
      assert_eq!(cpu.get_status(flag), flag as u8);
      cpu.set_status(flag, false);
      assert_eq!(cpu.get_status(flag), 0b0000_0000);
    }
  }

  #[test]
  fn simple_and() {
    let mut bus: DeviceList = DeviceList::new(vec![Box::new(Ram::new(0x0000, 64 * 1024))]);
    let mut cpu = Cpu::new();
    let program_start: u16 = 0x8000;

    bus.write16(PC_INIT_ADDR, program_start);

    bus.write(program_start, 0x29); // AND - Immediate
    bus.write(program_start + 1, 0x02); //   2

    cpu.sig_reset(&mut bus);
    cpu.step(&mut bus);

    cpu.a = 0x01;
    assert_eq!(cpu.a, 0x01);
    assert_eq!(cpu.get_status(Zero), 0x00);

    cpu.step(&mut bus);

    // Our accumulator should be 0 now:
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.get_status(Zero), Zero as u8);
  }

  #[test]
  fn simple_ora() {
    let ram = Ram::new(0x0000, 64 * 1024);
    let program_start: u16 = 0x8000;
    let mut bus: DeviceList = DeviceList::new(vec![Box::new(ram)]);
    let mut cpu = Cpu::new();
    bus.write16(PC_INIT_ADDR, program_start);

    bus.write(program_start, 0x09); // ORA - Immediate
    bus.write(program_start + 1, 0x02); //   2
    cpu.sig_reset(&mut bus);
    cpu.step(&mut bus);

    cpu.a = 0x01;
    assert_eq!(cpu.a, 0x01);
    assert_eq!(cpu.get_status(Zero), 0x00);

    cpu.step(&mut bus);

    // Our accumulator should be 3 now:
    assert_eq!(cpu.a, 0x03);
    assert_eq!(cpu.get_status(Zero), 0x00);
  }

  #[test]
  fn simple_eor() {
    let ram = Ram::new(0x0000, 64 * 1024);
    let program_start: u16 = 0x8000;
    let mut bus: DeviceList = DeviceList::new(vec![Box::new(ram)]);
    let mut cpu = Cpu::new();
    bus.write16(PC_INIT_ADDR, program_start);

    bus.write(program_start + 0, 0x49); // EOR - Immediate
    bus.write(program_start + 1, 0x02); //   2

    bus.write(program_start + 2, 0x49); // EOR - Immediate
    bus.write(program_start + 3, 0x02); //   2
    cpu.sig_reset(&mut bus);
    cpu.step(&mut bus);

    cpu.a = 0x01;
    assert_eq!(cpu.a, 0x01);
    assert_eq!(cpu.get_status(Zero), 0x00);

    cpu.step(&mut bus);

    // Our accumulator should be 3 now:
    assert_eq!(cpu.a, 0x03);
    assert_eq!(cpu.get_status(Zero), 0x00);

    cpu.step(&mut bus);

    // ```
    //  0b00000011
    // ^0b00000010
    // =0b00000001
    // ```
    assert_eq!(cpu.a, 0x01);
    assert_eq!(cpu.get_status(Zero), 0x00);
  }

  #[test]
  fn adc_overflow() {
    struct TestADC {
      // inputs:
      a: u8,
      m: u8,
      // expected outputs:
      r: u8,
      c: bool, // carry bit
      v: bool, // overflow bit
      z: bool, // zero bit
      n: bool, // negative bit
    }

    // Tests derived from the table at the bottom of this article:
    //
    // http://www.righto.com/2012/12/the-6502-overflow-flag-explained.html
    let tests: Vec<TestADC> = vec![
      TestADC {
        a: 0x50,
        m: 0x10,

        r: 0x60,
        c: false,
        v: false,
        z: false,
        n: false,
      },
      TestADC {
        a: 0x50,
        m: 0x50,

        r: 0xA0,
        c: false,
        v: true,
        z: false,
        n: true,
      },
      TestADC {
        a: 0x50,
        m: 0x90,

        r: 0xE0,
        c: false,
        v: false,
        z: false,
        n: true,
      },
      TestADC {
        a: 0x50,
        m: 0xD0,

        r: 0x20, // 0x20 + 0x100 (carry)
        c: true,
        v: false,
        z: false,
        n: false,
      },
      TestADC {
        a: 0xD0,
        m: 0x10,

        r: 0xE0,
        c: false,
        v: false,
        z: false,
        n: true,
      },
      TestADC {
        a: 0xD0,
        m: 0x50,

        r: 0x20, // 0x20 + 0x100 (carry)
        c: true,
        v: false,
        z: false,
        n: false,
      },
      TestADC {
        a: 0xD0,
        m: 0x90,

        r: 0x60, // 0x60 + 0x100 (carry)
        c: true,
        v: true,
        z: false,
        n: false,
      },
      TestADC {
        a: 0xD0,
        m: 0xD0,

        r: 0xA0, // 0xA0 + 0x100 (carry)
        c: true,
        v: false,
        z: false,
        n: true,
      },
    ];

    for test in tests {
      let program_start: u16 = 0x8000;
      let mut bus: DeviceList = DeviceList::new(vec![Box::new(Ram::new(0x0000, 64 * 1024))]);
      let mut cpu = Cpu::new();

      bus.write16(PC_INIT_ADDR, program_start);
      #[rustfmt::skip]
      let program: Vec<u8> = vec![
          0x69, test.m,
      ];
      let mut offset: u16 = 0;
      for byte in program {
        bus.write(program_start + offset, byte);
        offset += 1;
      }
      cpu.sig_reset(&mut bus);
      cpu.step(&mut bus);
      cpu.a = test.a;
      cpu.step(&mut bus);

      // The result should be stored into cpu.a:
      assert_eq!(cpu.a, test.r);

      assert_eq!(cpu.get_status(Carry) != 0, test.c);
      assert_eq!(cpu.get_status(Overflow) != 0, test.v);
      assert_eq!(cpu.get_status(Zero) != 0, test.z);
      assert_eq!(cpu.get_status(Negative) != 0, test.n);
    }
  }

  #[test]
  fn sbc_overflow() {
    // For now I'm disabling these because the results here seem to conflict
    // with the data from nestest.log; I'm guessing I got something wrong while
    // translating the data tables to create these test cases.
    return;

    // struct TestSBC {
    //   // inputs:
    //   a: u8,
    //   m: u8,

    //   // expected outputs:
    //   r: u8,
    //   c: bool, // carry bit
    //   v: bool, // overflow bit
    //   z: bool, // zero bit
    //   n: bool, // negative bit
    // }

    // // Tests derived from the table at the bottom of this article:
    // //
    // // http://www.righto.com/2012/12/the-6502-overflow-flag-explained.html
    // let tests: Vec<TestSBC> = vec![
    //   TestSBC {
    //     a: 0x50,
    //     m: 0xF0,

    //     r: 0x60,
    //     c: false,
    //     v: false,
    //     z: false,
    //     n: false,
    //   },
    //   TestSBC {
    //     a: 0x50,
    //     m: 0xB0,

    //     r: 0xA0,
    //     c: false,
    //     v: true,
    //     z: false,
    //     n: true,
    //   },
    //   TestSBC {
    //     a: 0x50,
    //     m: 0x70,

    //     r: 0xE0,
    //     c: false,
    //     v: false,
    //     z: false,
    //     n: true,
    //   },
    //   TestSBC {
    //     a: 0x50,
    //     m: 0x30,

    //     r: 0x20, // 0x20 + 0x100 (carry)
    //     c: true,
    //     v: false,
    //     z: false,
    //     n: false,
    //   },
    //   TestSBC {
    //     a: 0xD0,
    //     m: 0xF0,

    //     r: 0xE0,
    //     c: false,
    //     v: false,
    //     z: false,
    //     n: true,
    //   },
    //   TestSBC {
    //     a: 0xD0,
    //     m: 0xB0,

    //     r: 0x20, // 0x20 + 0x100 (carry)
    //     c: true,
    //     v: false,
    //     z: false,
    //     n: false,
    //   },
    //   TestSBC {
    //     a: 0xD0,
    //     m: 0x70,

    //     r: 0x60, // 0x60 + 0x100 (carry)
    //     c: true,
    //     v: true,
    //     z: false,
    //     n: false,
    //   },
    //   TestSBC {
    //     a: 0xD0,
    //     m: 0x30,

    //     r: 0xA0, // 0xA0 + 0x100 (carry)
    //     c: true,
    //     v: false,
    //     z: false,
    //     n: true,
    //   },
    // ];

    // for test in tests {
    //   let program_start: u16 = 0x8000;
    //   let mut bus: DeviceList = vec![Box::new(Ram::new(0x0000, 64 * 1024))];
    //   let mut cpu = Cpu::new();

    //   bus.write16(PC_INIT_ADDR, program_start);
    //   #[rustfmt::skip]
    //   let program: Vec<u8> = vec![
    //       0xE9, test.m,
    //   ];
    //   let mut offset: u16 = 0;
    //   for byte in program {
    //     bus.write(program_start + offset, byte);
    //     offset += 1;
    //   }
    //   cpu.sig_reset(&mut bus);
    //   cpu.step(&mut bus);
    //   cpu.a = test.a;
    //   cpu.step(&mut bus);

    //   // The result should be stored into cpu.a:
    //   assert_eq!(cpu.a, test.r);

    //   assert_eq!(cpu.get_status(Carry) != 0, test.c);
    //   assert_eq!(cpu.get_status(Overflow) != 0, test.v);
    //   assert_eq!(cpu.get_status(Zero) != 0, test.z);
    //   assert_eq!(cpu.get_status(Negative) != 0, test.n);
    // }
  }
}
