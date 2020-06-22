use crate::cpu6502::AddressingMode::*;
use crate::cpu6502::Instruction::*;
use crate::cpu6502::Operation;

pub struct DisassembledOperation {
  pub instruction_name: String,
  pub params: String,
  pub offset: u16,
}

pub fn disassemble(program: &Vec<u8>) -> Vec<DisassembledOperation> {
  let mut output: Vec<DisassembledOperation> = vec![];
  let mut pc = 0x0000;
  while pc < program.len() {
    let offset = pc;
    let operation: &Operation = program[pc].into();
    pc += 1;
    let instruction_name: String = match operation.instruction {
      ADC => "ADC",
      AND => "AND",
      ASL => "ASL",
      BCC => "BCC",
      BCS => "BCS",
      BEQ => "BEQ",
      BIT => "BIT",
      BMI => "BMI",
      BNE => "BNE",
      BPL => "BPL",
      BRK => "BRK",
      BVC => "BVC",
      BVS => "BVS",
      CLC => "CLC",
      CLD => "CLD",
      CLI => "CLI",
      CLV => "CLV",
      CMP => "CMP",
      CPX => "CPX",
      CPY => "CPY",
      DEC => "DEC",
      DEX => "DEX",
      DEY => "DEY",
      EOR => "EOR",
      INC => "INC",
      INX => "INX",
      INY => "INY",
      JMP => "JMP",
      JSR => "JSR",
      LDA => "LDA",
      LDX => "LDX",
      LDY => "LDY",
      LSR => "LSR",
      NOP => "NOP",
      ORA => "ORA",
      PHA => "PHA",
      PHP => "PHP",
      PLA => "PLA",
      PLP => "PLP",
      ROL => "ROL",
      ROR => "ROR",
      RTI => "RTI",
      RTS => "RTS",
      SBC => "SBC",
      SEC => "SEC",
      SED => "SED",
      SEI => "SEI",
      STA => "STA",
      STX => "STX",
      STY => "STY",
      TAX => "TAX",
      TAY => "TAY",
      TSX => "TSX",
      TXA => "TXA",
      TXS => "TXS",
      TYA => "TYA",
    }
    .into();
    let params: String = match &operation.addressing_mode {
      IMP => {
        // Implied; nothing to read:
        "".into()
      }
      IMM => {
        // Immediate; read one byte:
        let param = program[pc % program.len()];
        pc += 1;
        format!("#{:02X}", param)
      }
      ZP0 => {
        // Zero Page; read one byte:
        let param = program[pc % program.len()];
        pc += 1;
        format!("${:02X}", param)
      }
      ZPX => {
        // Zero Page with X offset; read one byte:
        let param = program[pc % program.len()];
        pc += 1;
        format!("${:02X},X", param)
      }
      ZPY => {
        // Zero Page with Y offset; read one byte:
        let param = program[pc % program.len()];
        pc += 1;
        format!("${:02X},Y", param)
      }
      ABS => {
        // Absolute; read two bytes:
        let lo = program[pc % program.len()] as u16;
        pc += 1;
        let hi = program[pc % program.len()] as u16;
        pc += 1;
        format!("${:04X}", (hi << 8) | lo)
      }
      ABX => {
        // Absolute, X; read two bytes:
        let lo = program[pc % program.len()] as u16;
        pc += 1;
        let hi = program[pc % program.len()] as u16;
        pc += 1;
        format!("${:04X},X", (hi << 8) | lo)
      }
      ABY => {
        // Absolute, Y; read two bytes:
        let lo = program[pc % program.len()] as u16;
        pc += 1;
        let hi = program[pc % program.len()] as u16;
        pc += 1;
        format!("${:04X},Y", (hi << 8) | lo)
      }
      IND => {
        // Indirect, Y; read four bytes:
        let lo = program[pc % program.len()] as u16;
        pc += 1;
        let hi = program[pc % program.len()] as u16;
        pc += 1;
        format!("(${:04X})", (hi << 8) | lo)
      }
      IZX => {
        // Indexed Indirect; read one byte:
        let param = program[pc % program.len()];
        pc += 1;
        format!("(${:02X},X)", param)
      }
      IZY => {
        // Indirect Indexed; read one byte:
        let param = program[pc % program.len()];
        pc += 1;
        format!("(${:02X}),Y", param)
      }
      ACC => {
        // Accumulator; nothing to read:
        "A".into()
      }
      REL => {
        // Relative; read one byte:
        let param = program[pc % program.len()];
        pc += 1;

        if param & 0x80 != 0 {
          // Get the inverted version of the offset by applying two's complement:
          let neg_offset = !(param as u16) + 1 & 0x00FF;
          format!("*-{:02}", neg_offset)
        } else {
          format!("*+{:02}", param)
        }
      }
    };

    output.push(DisassembledOperation {
      instruction_name,
      params,
      offset: offset as u16,
    });
  }

  output
}
