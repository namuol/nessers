use crate::bus::Bus;
use lazy_static::lazy_static;
use std::collections::HashMap;

/// 6502 Processor Status bits
///
/// See the "Processor Status" register description here:
///
/// - http://obelisk.me.uk/6502/registers.html
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

pub struct Processor {
  pub bus: Box<dyn Bus>,

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
  cycles_left: u8,
}

pub const STACK_START: u16 = 0x0100;
pub const STACK_SIZE: u8 = 0xFF;

/// An address that should contain a pointer to the start of our program
pub const PC_INIT_ADDR: u16 = 0xFFFC;

const IRQ_POINTER: u16 = 0xFFFE;
const NMI_POINTER: u16 = 0xFFFA;

impl Processor {
  pub fn new(bus: Box<dyn Bus>) -> Processor {
    Processor {
      bus,
      status: 0,
      a: 0,
      x: 0,
      y: 0,
      pc: 0,
      s: 0,
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

  pub fn step(&mut self) {
    loop {
      self.sig_clock();
      if self.cycles_left == 0 {
        return;
      }
    }
  }

  fn push(&mut self, data: u8) {
    self.bus.write(STACK_START + (self.s as u16), data);
    self.s -= 1;
  }

  fn pull(&mut self) -> u8 {
    let data = self.bus.read(STACK_START + (self.s as u16));
    self.s += 1;
    data
  }

  // SIGNALS:
  pub fn sig_clock(&mut self) {
    if self.cycles_left == 0 {
      let opcode = self.bus.read(self.pc);
      self.pc += 1;

      let operation: &Operation = opcode.into();

      self.cycles_left = operation.cycles;

      let address_mode_result = (operation.addressing_mode)(self);
      let instruction_result = (operation.instruction)(self, &address_mode_result.data);

      if address_mode_result.needs_extra_cycle && instruction_result.may_need_extra_cycle {
        self.cycles_left += 1;
      }
    }

    self.cycles_left -= 1;
  }

  pub fn sig_reset(&mut self) {
    self.a = 0x00;
    self.x = 0x00;
    self.y = 0x00;
    self.s = STACK_SIZE;
    self.status = 0x00 | (StatusFlag::Unused as u8);
    self.pc = self.bus.read16(PC_INIT_ADDR);

    self.cycles_left = 8;
  }

  pub fn sig_irq(&mut self) {
    if self.get_status(StatusFlag::DisableInterrupts) != 0x00 {
      let pc_hi: u8 = (self.pc >> 8) as u8;
      self.push(pc_hi);
      let pc_lo: u8 = (self.pc << 8) as u8;
      self.push(pc_lo);
      self.set_status(Break, false);
      self.set_status(Unused, true);
      self.set_status(DisableInterrupts, true);
      self.push(self.status);
      let irq_addr = self.bus.read16(IRQ_POINTER);
      self.pc = irq_addr;
      self.cycles_left = 7;
    }
  }

  pub fn sig_nmi(&mut self) {
    let pc_hi: u8 = (self.pc >> 8) as u8;
    self.push(pc_hi);
    let pc_lo: u8 = (self.pc << 8) as u8;
    self.push(pc_lo);
    self.set_status(Break, false);
    self.set_status(Unused, true);
    self.set_status(DisableInterrupts, true);
    self.push(self.status);
    let irq_addr = self.bus.read16(NMI_POINTER);
    self.pc = irq_addr;

    self.cycles_left = 8;
  }
}

struct Operation {
  pub addressing_mode: AddressingMode,
  pub instruction: Instruction,
  pub cycles: u8,
}

enum DataSourceKind {
  Accumulator,
  AbsoluteAddress,
  RelativeAddress,
  Implicit,
}
use DataSourceKind::*;

struct DataSource {
  kind: DataSourceKind,
  addr: u16,
}

impl DataSource {
  pub fn read(&self, cpu: &Processor) -> u8 {
    match self.kind {
      Accumulator => cpu.a,
      AbsoluteAddress => cpu.bus.read(self.addr),
      RelativeAddress => cpu.bus.read(cpu.pc + self.addr),
      Implicit => panic!("Cannot read from Implicit DataSource"),
    }
  }

  pub fn write(&self, cpu: &mut Processor, data: u8) {
    match self.kind {
      Accumulator => cpu.a = data,
      AbsoluteAddress => cpu.bus.write(self.addr, data),
      RelativeAddress => cpu.bus.write(cpu.pc + self.addr, data),
      Implicit => panic!("Cannot write to Implicit DataSource"),
    }
  }
}

struct AddressingModeResult {
  data: DataSource,
  needs_extra_cycle: bool,
}

/// An Addressing Mode ultimately provides some data to be used by an
/// instruction, either in the form of a constant, read-only byte value (`data`)
/// or an absolute address from which the data can be retrieved/written to
/// (`addr_abs`)
type AddressingMode = fn(&mut Processor) -> AddressingModeResult;

struct InstructionResult {
  may_need_extra_cycle: bool,
}
type Instruction = fn(&mut Processor, &DataSource) -> InstructionResult;

// INSTRUCTIONS ///////////////////////////////////////////////////////////////

// LOGICAL INSTRUCTIONS

/// AND
fn and(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  cpu.a = cpu.a & data.read(cpu);
  cpu.set_status(Zero, cpu.a == 0x00);
  cpu.set_status(Negative, cpu.a & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: true,
  }
}

/// Exclusive OR
fn eor(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  cpu.a = cpu.a ^ data.read(cpu);
  cpu.set_status(Zero, cpu.a == 0x00);
  cpu.set_status(Negative, cpu.a & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: true,
  }
}

/// Inclusive OR
fn ora(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  cpu.a = cpu.a | data.read(cpu);
  cpu.set_status(Zero, cpu.a == 0x00);
  cpu.set_status(Negative, cpu.a & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: true,
  }
}

/// Bit Test
fn bit(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu);
  let result = cpu.a & m;
  cpu.set_status(Zero, cpu.a == 0x00);

  // Bit 6 from memory value is copied to overflow flag (why?):
  cpu.set_status(Overflow, (0b_0100_0000 & m) != 0);

  cpu.set_status(Negative, (0b_1000_0000 & m) != 0);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

// LOAD/STORE OPERATIONS

/// Load Accumulator
fn lda(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu);
  cpu.a = m;
  cpu.set_status(Zero, m == 0);
  cpu.set_status(Negative, (0b_1000_0000 & m) != 0);
  InstructionResult {
    may_need_extra_cycle: true,
  }
}

/// Load X
fn ldx(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu);
  cpu.x = m;
  cpu.set_status(Zero, m == 0);
  cpu.set_status(Negative, (0b_1000_0000 & m) != 0);
  InstructionResult {
    may_need_extra_cycle: true,
  }
}

/// Load Y
fn ldy(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu);
  cpu.y = m;
  cpu.set_status(Zero, m == 0);
  cpu.set_status(Negative, (0b_1000_0000 & m) != 0);
  InstructionResult {
    may_need_extra_cycle: true,
  }
}

/// Store Accumulator
fn sta(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  data.write(cpu, cpu.a);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Store X
fn stx(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  data.write(cpu, cpu.x);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Store Y
fn sty(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  data.write(cpu, cpu.y);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

// Register Transfers

/// Transfer Accumulator to X
fn tax(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  cpu.x = cpu.a;

  cpu.set_status(Zero, cpu.a == 0x00);
  cpu.set_status(Negative, cpu.a & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Transfer Accumulator to Y
fn tay(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  cpu.y = cpu.a;

  cpu.set_status(Zero, cpu.a == 0x00);
  cpu.set_status(Negative, cpu.a & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Transfer X to Accumulator
fn txa(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  cpu.a = cpu.x;

  cpu.set_status(Zero, cpu.x == 0x00);
  cpu.set_status(Negative, cpu.x & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Transfer Y to Accumulator
fn tya(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  cpu.a = cpu.y;

  cpu.set_status(Zero, cpu.y == 0x00);
  cpu.set_status(Negative, cpu.y & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

// Stack Operations

/// Transfer Stack Pointer to X
fn tsx(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  cpu.x = cpu.s;

  cpu.set_status(Zero, cpu.s == 0x00);
  cpu.set_status(Negative, cpu.s & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Transfer X to Stack Pointer
fn txs(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  cpu.s = cpu.x;

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Push Accumulator
fn pha(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  cpu.push(cpu.a);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Push Processor Status
fn php(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  cpu.push(cpu.status);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Pull Accumulator
fn pla(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  cpu.a = cpu.pull();

  cpu.set_status(Zero, cpu.a == 0x00);
  cpu.set_status(Negative, cpu.a & 0b_1000_0000 != 0);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Pull Processor Status
fn plp(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  cpu.status = cpu.pull();

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

// Arithmetic

/// Add with Carry
fn adc(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  let a = cpu.a as u16;
  let m = data.read(cpu) as u16;
  let result = a + m + (cpu.get_status(Carry) as u16);
  {
    let overflow: u16 = (a ^ result) & !(a ^ m) & 0x0080;
    cpu.set_status(Overflow, overflow != 0);
  }
  cpu.set_status(Carry, result > 0xFF);
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x80) != 0);
  cpu.a = (result & 0x00FF) as u8;
  InstructionResult {
    may_need_extra_cycle: true,
  }
}

/// Subtract with Carry
fn sbc(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  let a = cpu.a as u16;
  // This implementation is identical to ADC, except we invert the lower 8 bits
  let m = (data.read(cpu) as u16) ^ 0x00FF;
  let result = a + m + (cpu.get_status(Carry) as u16) + 1;
  {
    let overflow: u16 = (a ^ result) & !(a ^ m) & 0x0080;
    cpu.set_status(Overflow, overflow != 0);
  }
  cpu.set_status(Carry, result > 0xFF);
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x80) != 0);
  cpu.a = (result & 0x00FF) as u8;
  InstructionResult {
    may_need_extra_cycle: true,
  }
}

/// Compare Accumulator
fn cmp(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  let a = cpu.a as u16;
  let m = data.read(cpu) as u16;
  let result = a - m;
  cpu.set_status(Carry, a >= m);
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x0080) != 0);
  InstructionResult {
    may_need_extra_cycle: true,
  }
}

/// Compare X
fn cpx(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  let x = cpu.x as u16;
  let m = data.read(cpu) as u16;
  let result = x - m;
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
fn cpy(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  let y = cpu.y as u16;
  let m = data.read(cpu) as u16;
  let result = y - m;
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
fn inc(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu) as u16;
  let result = m + 1;
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x0080) != 0);
  data.write(cpu, (result & 0x00FF) as u8);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Increment X
fn inx(cpu: &mut Processor, _data: &DataSource) -> InstructionResult {
  let result = (cpu.x as u16) + 1;
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x0080) != 0);
  cpu.x = (result & 0x00FF) as u8;
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Increment Y
fn iny(cpu: &mut Processor, _data: &DataSource) -> InstructionResult {
  let result = (cpu.y as u16) + 1;
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x0080) != 0);
  cpu.y = (result & 0x00FF) as u8;
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Decrement Memory
fn dec(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu) as u16;
  let result = m - 1;
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x0080) != 0);
  data.write(cpu, (result & 0x00FF) as u8);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Decrement X
fn dex(cpu: &mut Processor, _data: &DataSource) -> InstructionResult {
  let result = (cpu.x as u16) - 1;
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x0080) != 0);
  cpu.x = (result & 0x00FF) as u8;
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Decrement Y
fn dey(cpu: &mut Processor, _data: &DataSource) -> InstructionResult {
  let result = (cpu.y as u16) - 1;
  cpu.set_status(Zero, (result & 0x00FF) == 0);
  cpu.set_status(Negative, (result & 0x0080) != 0);
  cpu.y = (result & 0x00FF) as u8;
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

// Shifts

/// Arithmetic Shift Left
fn asl(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu);
  let result = m << 1; // equivalent to m * 2

  // We set the carry bit to the 7th bit from our data, since it was shifted
  // "out" of the result:
  cpu.set_status(Carry, m & 0x80 == 0x80);
  data.write(cpu, result);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Logical Shift Right
fn lsr(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu);
  let result = m >> 1; // equivalent to m / 2

  // We set the carry bit to the 0th bit from our data, since it was shifted
  // "out" of the result:
  cpu.set_status(Carry, m & 0x01 == 0x01);
  data.write(cpu, result);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Rotate Left
fn rol(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu);
  let old_bit_7 = m >> 7;
  let result = (m << 1) | old_bit_7;

  cpu.set_status(Carry, old_bit_7 != 0);
  data.write(cpu, result);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Rotate Right
fn ror(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  let m = data.read(cpu);
  let old_bit_0 = m & 0x01;
  let result = (m >> 1) | (old_bit_0 << 7);

  cpu.set_status(Carry, old_bit_0 != 0);
  data.write(cpu, result);

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Jumps & Calls

/// Jump
fn jmp(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  cpu.pc = data.addr;
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Jump to Subroutine
fn jsr(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  let return_addr = cpu.pc - 1;
  let return_hi: u8 = (return_addr >> 8) as u8;
  cpu.push(return_hi);
  let return_lo: u8 = (return_addr << 8) as u8;
  cpu.push(return_lo);

  cpu.pc = data.addr;
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Return from Subroutine
fn rts(cpu: &mut Processor, _data: &DataSource) -> InstructionResult {
  let return_lo = cpu.pull();
  let return_hi = cpu.pull();
  let return_addr = ((return_hi as u16) << 8) | return_lo as u16;
  cpu.pc = return_addr;
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

// Branches

/// Utility function for branching instructions.
///
/// Every branching instruction has the same characteristic, but operates on a
/// different condition.
fn branch_if(condition: bool, cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  if condition {
    // If we are branching, we use up an extra cycle
    cpu.cycles_left += 1;

    let new_pc = cpu.pc + data.addr;
    // If we're moving the program counter into a new page, we use one cycle in
    // _addition_ to the cycle we use to branch (totaling +2).
    //
    // We can detect if we are crossing pages by comparing the hi byte of the
    // new program counter with the hi bytes in the old program counter:
    if new_pc & 0xFF00 != cpu.pc & 0xFF00 {
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
fn bcc(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  branch_if(cpu.get_status(Carry) == 0, cpu, data)
}

/// Branch if Carry Set
fn bcs(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  branch_if(cpu.get_status(Carry) != 0, cpu, data)
}

/// Branch if Equal
fn beq(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  branch_if(cpu.get_status(Zero) != 0, cpu, data)
}

/// Branch if Minus
fn bmi(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  branch_if(cpu.get_status(Negative) != 0, cpu, data)
}

/// Branch if Positive
fn bpl(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  branch_if(cpu.get_status(Negative) == 0, cpu, data)
}

/// Branch if Not Equal
fn bne(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  branch_if(cpu.get_status(Zero) == 0, cpu, data)
}

/// Branch if Overflow Clear
fn bvc(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  branch_if(cpu.get_status(Overflow) == 0, cpu, data)
}

/// Branch if Overflow Set
fn bvs(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  branch_if(cpu.get_status(Overflow) != 0, cpu, data)
}

// Status Flag Changes

/// Clear carry
fn clc(cpu: &mut Processor, _data: &DataSource) -> InstructionResult {
  cpu.set_status(Carry, false);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Clear decimal mode
fn cld(cpu: &mut Processor, _data: &DataSource) -> InstructionResult {
  cpu.set_status(DecimalMode, false);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Clear interrupt disable
fn cli(cpu: &mut Processor, _data: &DataSource) -> InstructionResult {
  cpu.set_status(DisableInterrupts, false);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Clear overflow
fn clv(cpu: &mut Processor, _data: &DataSource) -> InstructionResult {
  cpu.set_status(Overflow, false);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Set carry
fn sec(cpu: &mut Processor, _data: &DataSource) -> InstructionResult {
  cpu.set_status(Carry, true);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Set decimal mode
fn sed(cpu: &mut Processor, _data: &DataSource) -> InstructionResult {
  cpu.set_status(DecimalMode, true);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Set interrupt disable
fn sei(cpu: &mut Processor, _data: &DataSource) -> InstructionResult {
  cpu.set_status(DisableInterrupts, true);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

// System Functions

/// Force an interrupt
fn brk(cpu: &mut Processor, _data: &DataSource) -> InstructionResult {
  let pc_hi: u8 = (cpu.pc >> 8) as u8;
  cpu.push(pc_hi);
  let pc_lo: u8 = (cpu.pc << 8) as u8;
  cpu.push(pc_lo);
  cpu.push(cpu.status);

  let irq_addr = cpu.bus.read16(IRQ_POINTER);
  cpu.pc = irq_addr;
  cpu.set_status(Break, true);
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// Return from interrupt
fn rti(cpu: &mut Processor, _data: &DataSource) -> InstructionResult {
  cpu.status = cpu.pull();

  let pc_hi = cpu.pull() as u16;
  let pc_lo = cpu.pull() as u16;
  cpu.pc = (pc_hi << 8) | pc_lo;

  let irq_addr = cpu.bus.read16(IRQ_POINTER);
  cpu.pc = irq_addr;

  InstructionResult {
    may_need_extra_cycle: false,
  }
}

/// No operation
fn nop(cpu: &mut Processor, _data: &DataSource) -> InstructionResult {
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
fn imp(_cpu: &mut Processor) -> AddressingModeResult {
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
fn imm(cpu: &mut Processor) -> AddressingModeResult {
  let addr_abs = cpu.pc;
  cpu.pc += 1;

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
fn zp0(cpu: &mut Processor) -> AddressingModeResult {
  // Read the first operand, constructing a 16-bit address within the zeroth
  // page:
  let addr_abs = (cpu.bus.read(cpu.pc) as u16) & 0x00FF;
  cpu.pc += 1;
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
fn zpx(cpu: &mut Processor) -> AddressingModeResult {
  // Read the first operand, constructing a 16-bit address within the zeroth
  // page:
  let addr_abs = ((cpu.x + cpu.bus.read(cpu.pc)) as u16) & 0x00FF;
  cpu.pc += 1;
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
fn zpy(cpu: &mut Processor) -> AddressingModeResult {
  // Read the first operand, constructing a 16-bit address within the zeroth
  // page:
  let addr_abs = ((cpu.y + cpu.bus.read(cpu.pc)) as u16) & 0x00FF;
  cpu.pc += 1;
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
fn abs(cpu: &mut Processor) -> AddressingModeResult {
  let addr_lo = cpu.bus.read(cpu.pc) as u16;
  cpu.pc += 1;
  let addr_hi = cpu.bus.read(cpu.pc) as u16;
  cpu.pc += 1;
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
fn abx(cpu: &mut Processor) -> AddressingModeResult {
  let addr_lo = cpu.bus.read(cpu.pc) as u16;
  cpu.pc += 1;
  let addr_hi = cpu.bus.read(cpu.pc) as u16;
  cpu.pc += 1;
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
fn aby(cpu: &mut Processor) -> AddressingModeResult {
  let addr_lo = cpu.bus.read(cpu.pc) as u16;
  cpu.pc += 1;
  let addr_hi = cpu.bus.read(cpu.pc) as u16;
  cpu.pc += 1;
  let addr_abs = ((addr_hi << 8) | addr_lo) + cpu.y as u16;

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
fn ind(cpu: &mut Processor) -> AddressingModeResult {
  let ptr_lo = cpu.bus.read(cpu.pc) as u16;
  cpu.pc += 1;
  let ptr_hi = cpu.bus.read(cpu.pc) as u16;
  cpu.pc += 1;
  let ptr = ptr_hi << 8 | ptr_lo;

  // The 6502 has a hardware bug where if you happen to have a pointer address
  // in memory that spans across pages (remember, pointers are 2 bytes, and
  // therefore it is possible for this to happen), it will not actually read the
  // hi byte of the address properly
  let addr_abs = if ptr_lo == 0x00FF {
    ((cpu.bus.read(ptr & 0xFF00) as u16) << 8) | cpu.bus.read(ptr) as u16
  } else {
    cpu.bus.read16(ptr)
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
fn izx(cpu: &mut Processor) -> AddressingModeResult {
  // Our pointer lives in the zeroth page, so we only need to read one byte
  let ptr = cpu.bus.read(cpu.pc) as u16 & 0x00FF;
  cpu.pc += 1;

  // We read X offset from this pointer
  let addr_abs = cpu.bus.read16(ptr + (cpu.x as u16) & 0x00FF);
  AddressingModeResult {
    data: DataSource {
      kind: AbsoluteAddress,
      addr: addr_abs,
    },
    needs_extra_cycle: false,
  }
}

/// (Indirect), Y
fn idy(cpu: &mut Processor) -> AddressingModeResult {
  // Our pointer lives in the zeroth page, so we only need to read one byte
  let ptr = cpu.bus.read(cpu.pc) as u16 & 0x00FF;
  cpu.pc += 1;

  let addr_abs = cpu.bus.read16(ptr) + cpu.y as u16;

  // We only read this here so we can check if we crossed a page:
  let addr_hi = cpu.bus.read(ptr + 1) as u16 & 0x00FF;
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
fn acc(cpu: &mut Processor) -> AddressingModeResult {
  AddressingModeResult {
    data: DataSource {
      kind: Accumulator,
      addr: 0x0000,
    },
    needs_extra_cycle: false,
  }
}

/// Relative
fn rel(cpu: &mut Processor) -> AddressingModeResult {
  let mut offset = cpu.bus.read(cpu.pc) as u16 & 0x00FF;
  cpu.pc += 1;

  // This ensures the binary arithmatic works out when adding this relative
  // address to our program counter.
  if offset & 0x80 != 0 {
    offset |= 0xFF00;
  }

  AddressingModeResult {
    data: DataSource {
      kind: RelativeAddress,
      addr: offset,
    },
    needs_extra_cycle: false,
  }
}

fn noop(_: &mut Processor, _: &DataSource) -> InstructionResult {
  InstructionResult {
    may_need_extra_cycle: false,
  }
}

const ILLEGAL_OPERATION: Operation = Operation {
  addressing_mode: imp,
  instruction: noop,
  cycles: 1,
};

// Generated the following hashmap by running this JS on
// http://www.obelisk.me.uk/6502/reference.html
//
// ```js
// addressing_map = {
//   'Absolute,X': 'abx',
//   'Absolute,Y': 'aby',
//   '(Indirect,X)': 'idx',
//   '(Indirect),Y': 'idy',
//   'Zero Page': 'zp0',
//   'Zero Page,X': 'zpx',
//   'Zero Page,Y': 'zpx',
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
//   addressing_mode: ${addressing_map[addressing_mode] || addressing_mode},
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
      instruction: adc,
      addressing_mode: imm,
      cycles: 2,
    },
    0x65 => Operation {
      instruction: adc,
      addressing_mode: zp0,
      cycles: 3,
    },
    0x75 => Operation {
      instruction: adc,
      addressing_mode: zpx,
      cycles: 4,
    },
    0x6D => Operation {
      instruction: adc,
      addressing_mode: abs,
      cycles: 4,
    },
    0x7D => Operation {
      instruction: adc,
      addressing_mode: abx,
      cycles: 4,
    },
    0x79 => Operation {
      instruction: adc,
      addressing_mode: aby,
      cycles: 4,
    },
    0x61 => Operation {
      instruction: adc,
      addressing_mode: izx,
      cycles: 6,
    },
    0x71 => Operation {
      instruction: adc,
      addressing_mode: idy,
      cycles: 5,
    },
    0x29 => Operation {
      instruction: and,
      addressing_mode: imm,
      cycles: 2,
    },
    0x25 => Operation {
      instruction: and,
      addressing_mode: zp0,
      cycles: 3,
    },
    0x35 => Operation {
      instruction: and,
      addressing_mode: zpx,
      cycles: 4,
    },
    0x2D => Operation {
      instruction: and,
      addressing_mode: abs,
      cycles: 4,
    },
    0x3D => Operation {
      instruction: and,
      addressing_mode: abx,
      cycles: 4,
    },
    0x39 => Operation {
      instruction: and,
      addressing_mode: aby,
      cycles: 4,
    },
    0x21 => Operation {
      instruction: and,
      addressing_mode: izx,
      cycles: 6,
    },
    0x31 => Operation {
      instruction: and,
      addressing_mode: idy,
      cycles: 5,
    },
    0x0A => Operation {
      instruction: asl,
      addressing_mode: acc,
      cycles: 2,
    },
    0x06 => Operation {
      instruction: asl,
      addressing_mode: zp0,
      cycles: 5,
    },
    0x16 => Operation {
      instruction: asl,
      addressing_mode: zpx,
      cycles: 6,
    },
    0x0E => Operation {
      instruction: asl,
      addressing_mode: abs,
      cycles: 6,
    },
    0x1E => Operation {
      instruction: asl,
      addressing_mode: abx,
      cycles: 7,
    },
    0x90 => Operation {
      instruction: bcc,
      addressing_mode: rel,
      cycles: 2,
    },
    0xB0 => Operation {
      instruction: bcs,
      addressing_mode: rel,
      cycles: 2,
    },
    0xF0 => Operation {
      instruction: beq,
      addressing_mode: rel,
      cycles: 2,
    },
    0x24 => Operation {
      instruction: bit,
      addressing_mode: zp0,
      cycles: 3,
    },
    0x2C => Operation {
      instruction: bit,
      addressing_mode: abs,
      cycles: 4,
    },
    0x30 => Operation {
      instruction: bmi,
      addressing_mode: rel,
      cycles: 2,
    },
    0xD0 => Operation {
      instruction: bne,
      addressing_mode: rel,
      cycles: 2,
    },
    0x10 => Operation {
      instruction: bpl,
      addressing_mode: rel,
      cycles: 2,
    },
    0x00 => Operation {
      instruction: brk,
      addressing_mode: imp,
      cycles: 7,
    },
    0x50 => Operation {
      instruction: bvc,
      addressing_mode: rel,
      cycles: 2,
    },
    0x70 => Operation {
      instruction: bvs,
      addressing_mode: rel,
      cycles: 2,
    },
    0x18 => Operation {
      instruction: clc,
      addressing_mode: imp,
      cycles: 2,
    },
    0xD8 => Operation {
      instruction: cld,
      addressing_mode: imp,
      cycles: 2,
    },
    0x58 => Operation {
      instruction: cli,
      addressing_mode: imp,
      cycles: 2,
    },
    0xB8 => Operation {
      instruction: clv,
      addressing_mode: imp,
      cycles: 2,
    },
    0xC9 => Operation {
      instruction: cmp,
      addressing_mode: imm,
      cycles: 2,
    },
    0xC5 => Operation {
      instruction: cmp,
      addressing_mode: zp0,
      cycles: 3,
    },
    0xD5 => Operation {
      instruction: cmp,
      addressing_mode: zpx,
      cycles: 4,
    },
    0xCD => Operation {
      instruction: cmp,
      addressing_mode: abs,
      cycles: 4,
    },
    0xDD => Operation {
      instruction: cmp,
      addressing_mode: abx,
      cycles: 4,
    },
    0xD9 => Operation {
      instruction: cmp,
      addressing_mode: aby,
      cycles: 4,
    },
    0xC1 => Operation {
      instruction: cmp,
      addressing_mode: izx,
      cycles: 6,
    },
    0xD1 => Operation {
      instruction: cmp,
      addressing_mode: idy,
      cycles: 5,
    },
    0xE0 => Operation {
      instruction: cpx,
      addressing_mode: imm,
      cycles: 2,
    },
    0xE4 => Operation {
      instruction: cpx,
      addressing_mode: zp0,
      cycles: 3,
    },
    0xEC => Operation {
      instruction: cpx,
      addressing_mode: abs,
      cycles: 4,
    },
    0xC0 => Operation {
      instruction: cpy,
      addressing_mode: imm,
      cycles: 2,
    },
    0xC4 => Operation {
      instruction: cpy,
      addressing_mode: zp0,
      cycles: 3,
    },
    0xCC => Operation {
      instruction: cpy,
      addressing_mode: abs,
      cycles: 4,
    },
    0xC6 => Operation {
      instruction: dec,
      addressing_mode: zp0,
      cycles: 5,
    },
    0xD6 => Operation {
      instruction: dec,
      addressing_mode: zpx,
      cycles: 6,
    },
    0xCE => Operation {
      instruction: dec,
      addressing_mode: abs,
      cycles: 6,
    },
    0xDE => Operation {
      instruction: dec,
      addressing_mode: abx,
      cycles: 7,
    },
    0xCA => Operation {
      instruction: dex,
      addressing_mode: imp,
      cycles: 2,
    },
    0x88 => Operation {
      instruction: dey,
      addressing_mode: imp,
      cycles: 2,
    },
    0x49 => Operation {
      instruction: eor,
      addressing_mode: imm,
      cycles: 2,
    },
    0x45 => Operation {
      instruction: eor,
      addressing_mode: zp0,
      cycles: 3,
    },
    0x55 => Operation {
      instruction: eor,
      addressing_mode: zpx,
      cycles: 4,
    },
    0x4D => Operation {
      instruction: eor,
      addressing_mode: abs,
      cycles: 4,
    },
    0x5D => Operation {
      instruction: eor,
      addressing_mode: abx,
      cycles: 4,
    },
    0x59 => Operation {
      instruction: eor,
      addressing_mode: aby,
      cycles: 4,
    },
    0x41 => Operation {
      instruction: eor,
      addressing_mode: izx,
      cycles: 6,
    },
    0x51 => Operation {
      instruction: eor,
      addressing_mode: idy,
      cycles: 5,
    },
    0xE6 => Operation {
      instruction: inc,
      addressing_mode: zp0,
      cycles: 5,
    },
    0xF6 => Operation {
      instruction: inc,
      addressing_mode: zpx,
      cycles: 6,
    },
    0xEE => Operation {
      instruction: inc,
      addressing_mode: abs,
      cycles: 6,
    },
    0xFE => Operation {
      instruction: inc,
      addressing_mode: abx,
      cycles: 7,
    },
    0xE8 => Operation {
      instruction: inx,
      addressing_mode: imp,
      cycles: 2,
    },
    0xC8 => Operation {
      instruction: iny,
      addressing_mode: imp,
      cycles: 2,
    },
    0x4C => Operation {
      instruction: jmp,
      addressing_mode: abs,
      cycles: 3,
    },
    0x6C => Operation {
      instruction: jmp,
      addressing_mode: ind,
      cycles: 5,
    },
    0x20 => Operation {
      instruction: jsr,
      addressing_mode: abs,
      cycles: 6,
    },
    0xA9 => Operation {
      instruction: lda,
      addressing_mode: imm,
      cycles: 2,
    },
    0xA5 => Operation {
      instruction: lda,
      addressing_mode: zp0,
      cycles: 3,
    },
    0xB5 => Operation {
      instruction: lda,
      addressing_mode: zpx,
      cycles: 4,
    },
    0xAD => Operation {
      instruction: lda,
      addressing_mode: abs,
      cycles: 4,
    },
    0xBD => Operation {
      instruction: lda,
      addressing_mode: abx,
      cycles: 4,
    },
    0xB9 => Operation {
      instruction: lda,
      addressing_mode: aby,
      cycles: 4,
    },
    0xA1 => Operation {
      instruction: lda,
      addressing_mode: izx,
      cycles: 6,
    },
    0xB1 => Operation {
      instruction: lda,
      addressing_mode: idy,
      cycles: 5,
    },
    0xA2 => Operation {
      instruction: ldx,
      addressing_mode: imm,
      cycles: 2,
    },
    0xA6 => Operation {
      instruction: ldx,
      addressing_mode: zp0,
      cycles: 3,
    },
    0xB6 => Operation {
      instruction: ldx,
      addressing_mode: zpx,
      cycles: 4,
    },
    0xAE => Operation {
      instruction: ldx,
      addressing_mode: abs,
      cycles: 4,
    },
    0xBE => Operation {
      instruction: ldx,
      addressing_mode: aby,
      cycles: 4,
    },
    0xA0 => Operation {
      instruction: ldy,
      addressing_mode: imm,
      cycles: 2,
    },
    0xA4 => Operation {
      instruction: ldy,
      addressing_mode: zp0,
      cycles: 3,
    },
    0xB4 => Operation {
      instruction: ldy,
      addressing_mode: zpx,
      cycles: 4,
    },
    0xAC => Operation {
      instruction: ldy,
      addressing_mode: abs,
      cycles: 4,
    },
    0xBC => Operation {
      instruction: ldy,
      addressing_mode: abx,
      cycles: 4,
    },
    0x4A => Operation {
      instruction: lsr,
      addressing_mode: acc,
      cycles: 2,
    },
    0x46 => Operation {
      instruction: lsr,
      addressing_mode: zp0,
      cycles: 5,
    },
    0x56 => Operation {
      instruction: lsr,
      addressing_mode: zpx,
      cycles: 6,
    },
    0x4E => Operation {
      instruction: lsr,
      addressing_mode: abs,
      cycles: 6,
    },
    0x5E => Operation {
      instruction: lsr,
      addressing_mode: abx,
      cycles: 7,
    },
    0xEA => Operation {
      instruction: nop,
      addressing_mode: imp,
      cycles: 2,
    },
    0x09 => Operation {
      instruction: ora,
      addressing_mode: imm,
      cycles: 2,
    },
    0x05 => Operation {
      instruction: ora,
      addressing_mode: zp0,
      cycles: 3,
    },
    0x15 => Operation {
      instruction: ora,
      addressing_mode: zpx,
      cycles: 4,
    },
    0x0D => Operation {
      instruction: ora,
      addressing_mode: abs,
      cycles: 4,
    },
    0x1D => Operation {
      instruction: ora,
      addressing_mode: abx,
      cycles: 4,
    },
    0x19 => Operation {
      instruction: ora,
      addressing_mode: aby,
      cycles: 4,
    },
    0x01 => Operation {
      instruction: ora,
      addressing_mode: izx,
      cycles: 6,
    },
    0x11 => Operation {
      instruction: ora,
      addressing_mode: idy,
      cycles: 5,
    },
    0x48 => Operation {
      instruction: pha,
      addressing_mode: imp,
      cycles: 3,
    },
    0x08 => Operation {
      instruction: php,
      addressing_mode: imp,
      cycles: 3,
    },
    0x68 => Operation {
      instruction: pla,
      addressing_mode: imp,
      cycles: 4,
    },
    0x28 => Operation {
      instruction: plp,
      addressing_mode: imp,
      cycles: 4,
    },
    0x2A => Operation {
      instruction: rol,
      addressing_mode: acc,
      cycles: 2,
    },
    0x26 => Operation {
      instruction: rol,
      addressing_mode: zp0,
      cycles: 5,
    },
    0x36 => Operation {
      instruction: rol,
      addressing_mode: zpx,
      cycles: 6,
    },
    0x2E => Operation {
      instruction: rol,
      addressing_mode: abs,
      cycles: 6,
    },
    0x3E => Operation {
      instruction: rol,
      addressing_mode: abx,
      cycles: 7,
    },
    0x6A => Operation {
      instruction: ror,
      addressing_mode: acc,
      cycles: 2,
    },
    0x66 => Operation {
      instruction: ror,
      addressing_mode: zp0,
      cycles: 5,
    },
    0x76 => Operation {
      instruction: ror,
      addressing_mode: zpx,
      cycles: 6,
    },
    0x6E => Operation {
      instruction: ror,
      addressing_mode: abs,
      cycles: 6,
    },
    0x7E => Operation {
      instruction: ror,
      addressing_mode: abx,
      cycles: 7,
    },
    0x40 => Operation {
      instruction: rti,
      addressing_mode: imp,
      cycles: 6,
    },
    0x60 => Operation {
      instruction: rts,
      addressing_mode: imp,
      cycles: 6,
    },
    0xE9 => Operation {
      instruction: sbc,
      addressing_mode: imm,
      cycles: 2,
    },
    0xE5 => Operation {
      instruction: sbc,
      addressing_mode: zp0,
      cycles: 3,
    },
    0xF5 => Operation {
      instruction: sbc,
      addressing_mode: zpx,
      cycles: 4,
    },
    0xED => Operation {
      instruction: sbc,
      addressing_mode: abs,
      cycles: 4,
    },
    0xFD => Operation {
      instruction: sbc,
      addressing_mode: abx,
      cycles: 4,
    },
    0xF9 => Operation {
      instruction: sbc,
      addressing_mode: aby,
      cycles: 4,
    },
    0xE1 => Operation {
      instruction: sbc,
      addressing_mode: izx,
      cycles: 6,
    },
    0xF1 => Operation {
      instruction: sbc,
      addressing_mode: idy,
      cycles: 5,
    },
    0x38 => Operation {
      instruction: sec,
      addressing_mode: imp,
      cycles: 2,
    },
    0xF8 => Operation {
      instruction: sed,
      addressing_mode: imp,
      cycles: 2,
    },
    0x78 => Operation {
      instruction: sei,
      addressing_mode: imp,
      cycles: 2,
    },
    0x85 => Operation {
      instruction: sta,
      addressing_mode: zp0,
      cycles: 3,
    },
    0x95 => Operation {
      instruction: sta,
      addressing_mode: zpx,
      cycles: 4,
    },
    0x8D => Operation {
      instruction: sta,
      addressing_mode: abs,
      cycles: 4,
    },
    0x9D => Operation {
      instruction: sta,
      addressing_mode: abx,
      cycles: 5,
    },
    0x99 => Operation {
      instruction: sta,
      addressing_mode: aby,
      cycles: 5,
    },
    0x81 => Operation {
      instruction: sta,
      addressing_mode: izx,
      cycles: 6,
    },
    0x91 => Operation {
      instruction: sta,
      addressing_mode: idy,
      cycles: 6,
    },
    0x86 => Operation {
      instruction: stx,
      addressing_mode: zp0,
      cycles: 3,
    },
    0x96 => Operation {
      instruction: stx,
      addressing_mode: zpx,
      cycles: 4,
    },
    0x8E => Operation {
      instruction: stx,
      addressing_mode: abs,
      cycles: 4,
    },
    0x84 => Operation {
      instruction: sty,
      addressing_mode: zp0,
      cycles: 3,
    },
    0x94 => Operation {
      instruction: sty,
      addressing_mode: zpx,
      cycles: 4,
    },
    0x8C => Operation {
      instruction: sty,
      addressing_mode: abs,
      cycles: 4,
    },
    0xAA => Operation {
      instruction: tax,
      addressing_mode: imp,
      cycles: 2,
    },
    0xA8 => Operation {
      instruction: tay,
      addressing_mode: imp,
      cycles: 2,
    },
    0xBA => Operation {
      instruction: tsx,
      addressing_mode: imp,
      cycles: 2,
    },
    0x8A => Operation {
      instruction: txa,
      addressing_mode: imp,
      cycles: 2,
    },
    0x9A => Operation {
      instruction: txs,
      addressing_mode: imp,
      cycles: 2,
    },
    0x98 => Operation {
      instruction: tya,
      addressing_mode: imp,
      cycles: 2,
    }
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
  use crate::ram::Ram;

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
  impl Bus for DummyBus {
    fn write(&mut self, _: u16, _: u8) {}
    fn read(&self, _: u16) -> u8 {
      0x00
    }
  }

  #[test]
  fn get_status() {
    let mut cpu = Processor::new(Box::new(DummyBus {}));

    for flag in &ALL_FLAGS {
      assert_eq!(cpu.get_status(*flag), 0b0000);
    }

    for flag in &ALL_FLAGS {
      let flag = *flag;
      cpu.status = flag as u8;
      for other_flag in &ALL_FLAGS {
        let other_flag = *other_flag;
        if flag == other_flag {
          assert_eq!(cpu.get_status(other_flag), (flag as u8));
        } else {
          assert_eq!(cpu.get_status(other_flag), 0b0000);
        }
      }
    }
  }

  #[test]
  fn set_status() {
    let mut cpu = Processor::new(Box::new(DummyBus {}));

    for flag in &ALL_FLAGS {
      let flag = *flag;
      assert_eq!(cpu.get_status(flag), 0b0000);
      cpu.set_status(flag, true);
      assert_eq!(cpu.get_status(flag), flag as u8);
      cpu.set_status(flag, false);
      assert_eq!(cpu.get_status(flag), 0b0000);
    }
  }

  #[test]
  fn simple_and() {
    let mut cpu = Processor::new(Box::new(Ram::new()));
    let program_start: u16 = STACK_START + STACK_SIZE as u16 + 1;

    cpu.bus.write16(PC_INIT_ADDR, program_start);

    cpu.bus.write(program_start, 0x29); // AND - Immediate
    cpu.bus.write(program_start + 1, 0x02); //   2

    cpu.sig_reset();
    cpu.step();

    cpu.a = 0x01;
    assert_eq!(cpu.a, 0x01);
    assert_eq!(cpu.get_status(Zero), 0x00);

    cpu.step();

    // Our accumulator should be 0 now:
    assert_eq!(cpu.a, 0x00);
    assert_eq!(cpu.get_status(Zero), Zero as u8);
  }

  #[test]
  fn simple_ora() {
    let mut ram = Ram::new();
    let program_start: u16 = STACK_START + STACK_SIZE as u16 + 1;

    ram.write16(PC_INIT_ADDR, program_start);

    ram.buf[program_start as usize] = 0x09; // ORA - Immediate
    ram.buf[program_start as usize + 1] = 0x02; //   2

    let mut cpu = Processor::new(Box::new(ram));
    cpu.sig_reset();
    cpu.step();

    cpu.a = 0x01;
    assert_eq!(cpu.a, 0x01);
    assert_eq!(cpu.get_status(Zero), 0x00);

    cpu.step();

    // Our accumulator should be 3 now:
    assert_eq!(cpu.a, 0x03);
    assert_eq!(cpu.get_status(Zero), 0x00);
  }

  #[test]
  fn simple_eor() {
    let mut ram = Ram::new();
    let program_start: u16 = STACK_START + STACK_SIZE as u16 + 1;

    ram.write16(PC_INIT_ADDR, program_start);

    ram.buf[program_start as usize + 0] = 0x49; // EOR - Immediate
    ram.buf[program_start as usize + 1] = 0x02; //   2

    ram.buf[program_start as usize + 2] = 0x49; // EOR - Immediate
    ram.buf[program_start as usize + 3] = 0x02; //   2

    let mut cpu = Processor::new(Box::new(ram));
    cpu.sig_reset();
    cpu.step();

    cpu.a = 0x01;
    assert_eq!(cpu.a, 0x01);
    assert_eq!(cpu.get_status(Zero), 0x00);

    cpu.step();

    // Our accumulator should be 3 now:
    assert_eq!(cpu.a, 0x03);
    assert_eq!(cpu.get_status(Zero), 0x00);

    cpu.step();

    //  0b00000011
    // ^0b00000010
    // =0b00000001
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
      let program_start: u16 = STACK_START + STACK_SIZE as u16 + 1;
      let mut cpu = Processor::new(Box::new(Ram::new()));
      cpu.bus.write16(PC_INIT_ADDR, program_start);
      #[rustfmt::skip]
      let program: Vec<u8> = vec![
          0x69, test.m,
      ];
      let mut offset: u16 = 0;
      for byte in program {
        cpu.bus.write(program_start + offset, byte);
        offset += 1;
      }
      cpu.sig_reset();
      cpu.step();
      cpu.a = test.a;
      cpu.step();

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
    struct TestSBC {
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
    let tests: Vec<TestSBC> = vec![
      TestSBC {
        a: 0x50,
        m: 0xF0,

        r: 0x60,
        c: false,
        v: false,
        z: false,
        n: false,
      },
      TestSBC {
        a: 0x50,
        m: 0xB0,

        r: 0xA0,
        c: false,
        v: true,
        z: false,
        n: true,
      },
      TestSBC {
        a: 0x50,
        m: 0x70,

        r: 0xE0,
        c: false,
        v: false,
        z: false,
        n: true,
      },
      TestSBC {
        a: 0x50,
        m: 0x30,

        r: 0x20, // 0x20 + 0x100 (carry)
        c: true,
        v: false,
        z: false,
        n: false,
      },
      TestSBC {
        a: 0xD0,
        m: 0xF0,

        r: 0xE0,
        c: false,
        v: false,
        z: false,
        n: true,
      },
      TestSBC {
        a: 0xD0,
        m: 0xB0,

        r: 0x20, // 0x20 + 0x100 (carry)
        c: true,
        v: false,
        z: false,
        n: false,
      },
      TestSBC {
        a: 0xD0,
        m: 0x70,

        r: 0x60, // 0x60 + 0x100 (carry)
        c: true,
        v: true,
        z: false,
        n: false,
      },
      TestSBC {
        a: 0xD0,
        m: 0x30,

        r: 0xA0, // 0xA0 + 0x100 (carry)
        c: true,
        v: false,
        z: false,
        n: true,
      },
    ];

    for test in tests {
      let program_start: u16 = STACK_START + STACK_SIZE as u16 + 1;
      let mut cpu = Processor::new(Box::new(Ram::new()));
      cpu.bus.write16(PC_INIT_ADDR, program_start);
      #[rustfmt::skip]
      let program: Vec<u8> = vec![
          0xE9, test.m,
      ];
      let mut offset: u16 = 0;
      for byte in program {
        cpu.bus.write(program_start + offset, byte);
        offset += 1;
      }
      cpu.sig_reset();
      cpu.step();
      cpu.a = test.a;
      cpu.step();

      // The result should be stored into cpu.a:
      assert_eq!(cpu.a, test.r);

      assert_eq!(cpu.get_status(Carry) != 0, test.c);
      assert_eq!(cpu.get_status(Overflow) != 0, test.v);
      assert_eq!(cpu.get_status(Zero) != 0, test.z);
      assert_eq!(cpu.get_status(Negative) != 0, test.n);
    }
  }
}
