// pub enum AddressingMode {}
// pub enum CycleCount {}

// pub struct OpCode {
//   pub addressing_mode: AddressingMode,
//   pub cycles: CycleCount,
// }

/// 6502 Status Register bit flags
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

pub struct Processor {
  status: u8,
  accumulator: u8,
  x: u8,
  y: u8,
  program_counter: u16,
  stack_pointer: u8,
}

impl Processor {
  pub fn new() -> Processor {
    Processor {
      status: 0,
      accumulator: 0,
      x: 0,
      y: 0,
      program_counter: 0,
      stack_pointer: 0,
    }
  }

  pub fn get_flag(&self, flag: StatusFlag) -> u8 {
    self.status & (flag as u8)
  }
  pub fn set_flag(&mut self, flag: StatusFlag, value: bool) {
    if value {
      self.status |= flag as u8;
    } else {
      self.status &= !(flag as u8);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::cpu6502::StatusFlag::*;
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

  #[test]
  fn get_flag() {
    let mut cpu = Processor {
      status: 0,
      accumulator: 0,
      x: 0,
      y: 0,
      program_counter: 0,
      stack_pointer: 0,
    };

    for flag in &ALL_FLAGS {
      assert_eq!(cpu.get_flag(*flag), 0b0000);
    }

    for flag in &ALL_FLAGS {
      let flag = *flag;
      cpu.status = flag as u8;
      for other_flag in &ALL_FLAGS {
        let other_flag = *other_flag;
        if flag == other_flag {
          assert_eq!(cpu.get_flag(other_flag), (flag as u8));
        } else {
          assert_eq!(cpu.get_flag(other_flag), 0b0000);
        }
      }
    }
  }

  #[test]
  fn set_flag() {
    let mut cpu = Processor {
      status: 0,
      accumulator: 0,
      x: 0,
      y: 0,
      program_counter: 0,
      stack_pointer: 0,
    };

    for flag in &ALL_FLAGS {
      let flag = *flag;
      assert_eq!(cpu.get_flag(flag), 0b0000);
      cpu.set_flag(flag, true);
      assert_eq!(cpu.get_flag(flag), flag as u8);
      cpu.set_flag(flag, false);
      assert_eq!(cpu.get_flag(flag), 0b0000);
    }
  }
}
