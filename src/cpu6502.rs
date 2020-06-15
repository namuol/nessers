use crate::bus::Bus;

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

      let operation: Operation = opcode.into();

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

  pub fn sig_interrupt_request(&mut self) {
    if self.get_status(StatusFlag::DisableInterrupts) != 0x00 {
      self.sig_non_maskable_interrupt_request();
      return;
    }
  }

  pub fn sig_non_maskable_interrupt_request(&mut self) {
    todo!();
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
      RelativeAddress => todo!(),
    }
  }

  pub fn write(&self, cpu: &mut Processor, data: u8) {
    match self.kind {
      Accumulator => cpu.a = data,
      AbsoluteAddress => cpu.bus.write(self.addr, data),
      RelativeAddress => todo!(),
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

/// Stack Operations

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

// ADDRESSING MODES ///////////////////////////////////////////////////////////

/// Implied addressing
///
/// Nothing to do here, but some implied operations operate on the accumulator,
/// so we fetch that data here
fn imp(_cpu: &mut Processor) -> AddressingModeResult {
  AddressingModeResult {
    data: DataSource {
      kind: Accumulator,
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

// /// Zero Page addressing
// ///
// /// Read a byte at an address in the zeroth page; i.e. from one of the first 256
// /// bytes in memory
// fn zp0(cpu: &mut Processor) -> AddressingModeResult {
//   // Read the first operand, constructing a 16-bit address within the zeroth
//   // page:
//   let addr_abs = (cpu.bus.read(cpu.pc) as u16) & 0x00FF;
//   cpu.pc += 1;
//   AddressingModeResult {
//     data: 0x00,
//     addr_abs,
//     needs_extra_cycle: false,
//   }
// }

// /// Zero Page addressing, with X address offset
// ///
// /// Read a byte at an address in the zeroth page + X; i.e. starting from X, plus
// /// 0-255
// fn zpx(cpu: &mut Processor) -> AddressingModeResult {
//   // Read the first operand, constructing a 16-bit address within the zeroth
//   // page:
//   let addr_abs = ((cpu.x + cpu.bus.read(cpu.pc)) as u16) & 0x00FF;
//   cpu.pc += 1;
//   AddressingModeResult {
//     data: 0x00,
//     addr_abs,
//     needs_extra_cycle: false,
//   }
// }

// /// Zero Page addressing, with Y address offset
// ///
// /// Read a byte at an address in the zeroth page + Y; i.e. starting from Y, plus
// /// 0-255
// fn zpy(cpu: &mut Processor) -> AddressingModeResult {
//   // Read the first operand, constructing a 16-bit address within the zeroth
//   // page:
//   let addr_abs = ((cpu.y + cpu.bus.read(cpu.pc)) as u16) & 0x00FF;
//   cpu.pc += 1;
//   AddressingModeResult {
//     data: 0x00,
//     addr_abs,
//     needs_extra_cycle: false,
//   }
// }

// /// Absolute addressing
// ///
// /// Read a full 16-bit address from the current program counter + 1
// fn abs(cpu: &mut Processor) -> AddressingModeResult {
//   let addr_lo = cpu.bus.read(cpu.pc) as u16;
//   cpu.pc += 1;
//   let addr_hi = cpu.bus.read(cpu.pc) as u16;
//   cpu.pc += 1;
//   AddressingModeResult {
//     data: 0x00,
//     addr_abs: ((addr_hi << 8) | addr_lo),
//     needs_extra_cycle: false,
//   }
// }

// /// Absolute addressing + X
// ///
// /// Read a full 16-bit address from the current program counter + 1, then apply
// /// an offset of X
// fn abx(cpu: &mut Processor) -> AddressingModeResult {
//   let addr_lo = cpu.bus.read(cpu.pc) as u16;
//   cpu.pc += 1;
//   let addr_hi = cpu.bus.read(cpu.pc) as u16;
//   cpu.pc += 1;
//   let addr_abs = ((addr_hi << 8) | addr_lo) + cpu.x as u16;

//   // If our hi byte is changed after we've added X, then it has changed due to
//   // overflow which means we are crossing a page. When we cross a page, we may
//   // need an extra cycle:
//   let needs_extra_cycle = addr_abs & 0xFF00 != (addr_hi << 8);

//   AddressingModeResult {
//     data: 0x00,
//     addr_abs,
//     needs_extra_cycle,
//   }
// }
// /// Absolute addressing + Y
// ///
// /// Read a full 16-bit address from the current program counter + 1, then apply
// /// an offset of Y
// fn aby(cpu: &mut Processor) -> AddressingModeResult {
//   let addr_lo = cpu.bus.read(cpu.pc) as u16;
//   cpu.pc += 1;
//   let addr_hi = cpu.bus.read(cpu.pc) as u16;
//   cpu.pc += 1;
//   let addr_abs = ((addr_hi << 8) | addr_lo) + cpu.y as u16;

//   // If our hi byte is changed after we've added Y, then it has changed due to
//   // overflow which means we are crossing a page. When we cross a page, we may
//   // need an extra cycle:
//   let needs_extra_cycle = addr_abs & 0xFF00 != (addr_hi << 8);

//   AddressingModeResult {
//     data: 0x00,
//     addr_abs,
//     needs_extra_cycle,
//   }
// }

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

impl From<u8> for Operation {
  fn from(opcode: u8) -> Self {
    if opcode == 0x69 {
      return Operation {
        instruction: and,
        addressing_mode: imm,
        cycles: 2,
      };
    }

    if opcode == 0x09 {
      return Operation {
        instruction: ora,
        addressing_mode: imm,
        cycles: 2,
      };
    }
    ILLEGAL_OPERATION
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
    fn write(&mut self, _: u16, _: u8) {
      todo!()
    }
    fn read(&self, _: u16) -> u8 {
      todo!()
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

    cpu.bus.write(program_start, 0x69); // AND - Immediate
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
}
