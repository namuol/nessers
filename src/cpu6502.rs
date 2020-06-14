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
  pub acc: u8,
  /// X Register
  pub x: u8,
  /// Y Register
  pub y: u8,
  /// Program Counter
  pub pc: u16,
  /// Stack Pointer
  pub sp: u8,

  /// The numbers of cycles remaining for the current operation
  cycles_left: u8,
}

impl Processor {
  pub fn new(bus: Box<dyn Bus>) -> Processor {
    Processor {
      bus,
      status: 0,
      acc: 0,
      x: 0,
      y: 0,
      pc: 0,
      sp: 0,
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

  // SIGNALS:
  pub fn clock(&mut self) {
    if self.cycles_left == 0 {
      let opcode = self.bus.read(self.pc);
      self.pc += 1;

      let operation: Operation = opcode.into();

      let address_mode_result = (operation.addressing_mode)(self);
      let instruction_result = (operation.instruction)(self, &address_mode_result.data);

      if address_mode_result.needs_extra_cycle && instruction_result.needs_extra_cycle {
        self.cycles_left += 1;
      }
    }

    self.cycles_left -= 1;
  }

  pub fn reset(&mut self) {
    todo!();
  }

  pub fn interrupt_request(&mut self) {
    todo!();
  }

  pub fn non_maskable_interrupt_request(&mut self) {
    todo!();
  }
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
      Accumulator => cpu.acc,
      AbsoluteAddress => cpu.bus.read(self.addr),
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
  needs_extra_cycle: bool,
}
type Instruction = fn(&mut Processor, &DataSource) -> InstructionResult;

fn and(cpu: &mut Processor, data: &DataSource) -> InstructionResult {
  cpu.acc = cpu.acc & data.read(cpu);
  cpu.set_status(Zero, cpu.acc == 0x00);
  cpu.set_status(Negative, cpu.acc & 0x80 != 0x00);

  InstructionResult {
    needs_extra_cycle: true,
  }
}

struct Operation {
  pub addressing_mode: AddressingMode,
  pub instruction: Instruction,
}

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
  todo!()
}

const ILLEGAL_OPERATION: Operation = Operation {
  addressing_mode: imp,
  instruction: noop,
};

impl From<u8> for Operation {
  fn from(opcode: u8) -> Self {
    ILLEGAL_OPERATION
  }
}

#[cfg(test)]
mod tests {
  use super::*;
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
}
