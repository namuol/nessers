use crate::bus::Bus;
use crate::bus_device::BusDevice;
use crate::cart::Cart;
use crate::cpu6502::Cpu;
use crate::disassemble;
use crate::mirror::Mirror;
use crate::palette::{Color, Palette};
use crate::ppu::Ppu;
use crate::ram::Ram;

pub struct Nes {
  pub cpu: Cpu,
  pub ppu: Ppu,
  tick: u64,
  ram: Ram,
  ram_mirror: Mirror,
  ppu_registers_mirror: Mirror,
  cart: Cart,
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

    let cart = Cart::from_file(cart_filename)?;

    Ok(Nes {
      tick: 0,
      cpu,
      ppu,
      cart,
      ram_mirror,
      ram,
      ppu_registers_mirror,
    })
  }

  pub fn clock(&mut self) {
    self.ppu.clock();
    if self.tick % 3 == 0 {
      // Is there a shorthand way to run a method on a field by cloning it and
      // replacing its value with the cloned object?
      let cpu = &mut self.cpu.clone();
      cpu.clock(self);
      self.cpu = *cpu;
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

  pub fn frame(&mut self) {
    loop {
      self.clock();
      if self.ppu.frame_complete == true {
        return;
      }
    }
  }

  /// In order to render the pattern table, we need to understand how pattern
  /// data is structured in memory.
  ///
  /// On the NES, each pixel of a sprite is 2 bits, allowing up to 4 unique
  /// colors (including 0 which is always "transparent").
  ///
  /// A tile consists of an 8x8 grid of 2-bit pixels.
  ///
  /// It can help to picture a tile as something like this:
  ///
  /// ```
  /// 0, 1, 2, 3, 3, 2, 1, 0
  /// ...7 more rows like this...
  /// ```
  ///
  /// You might at first assume that these pixels are stored in the following
  /// way in memory (it'll be clear why I'm using binary notation here later):
  ///
  /// ```
  /// 0,    1,    2,    3,    3,    2,    1,    0
  /// 0b00, 0b01, 0b10, 0b11, 0b11, 0b10, 0b01, 0b00
  /// ...7 more rows like this...
  /// ```
  ///
  /// Written in _bytes_ (the unit we're used to reading one at a time) this
  /// would look like this:
  ///
  /// ```
  ///   0,1,2,3,    3,2,1,0
  /// 0b00011011, 0b11100100
  /// ...7 more rows like this...
  /// ```
  ///
  /// So in this form, a tile would be a sequence of 64 * 2-bit pixels, or 128
  /// bits = 16 bytes.
  ///
  /// This might seem fine and intuitive, until you actually go to _read_ the
  /// pixel at a specific coordinate within the data.
  ///
  /// For instance, let's say we wanted to get the pixel at x=3, y=3.
  ///
  /// We would first need to determine which _byte_ to read since I can only
  /// read one byte at a time.
  ///
  /// Then we'd need to perform some bit-operations on the byte to mask out the
  /// bits that aren't important to us, and then _finally_ we'd need to _shift_
  /// the bits such that only the 2-bit pixel we care about is selected.
  ///
  /// There's a better way: Bit-planes!
  ///
  /// Since our pixels are 2 bits each, we can _split_ our 8x8 2-bit grid in
  /// half such that the 8 bytes correspond to the _least significant bit_ of
  /// each of the 8x8=64 bits in the tile, and the next 8x8=64 bits correspond
  /// to the _most significant bit_ of each pixel in the tile.
  ///
  /// Concretely, the first 8 pixels (`0, 1, 2, 3, 3, 2, 1, 0`) could be
  /// represented like this in the pattern table memory:
  ///
  /// ```
  ///       2-bit number:  0   1   2   3   3   2   1   0
  ///      binary number: 00  01  10  11  11  10  01  00
  /// lsb (offset by  0):  0,  1,  0,  1,  1,  0,  1,  0
  ///                     ...rest of the lsb tile rows...
  /// msb (offset by 64): 0 , 0 , 1 , 1 , 1 , 1 , 0 , 0
  ///                     ...rest of the msb tile rows...
  /// ```
  ///
  /// So now if we want to render a tile, we can simply read two bytes at a time
  /// for each 8-pixel wide row of pixels, and to determine the 2-bit color of
  /// each column, we can mask all but the last bit from each byte we read and
  /// add them together appropriately (0b0<lsb> & 0b<msb>0, or more easily
  /// 0x0<lsb> + 0x0<msb>) to get our 2-bit color palette index.
  ///
  /// Whew!
  pub fn render_pattern_table(&mut self, table_number: u16, palette: u8) -> [[u8; 4]; 128 * 128] {
    let mut result = [[0x00, 0x00, 0x00, 0xFF]; 128 * 128];
    // We want to render 16x16 tiles
    for tile_y in 0..16 {
      for tile_x in 0..16 {
        // Even though a tile is 8x8, each tile is actually 16 bits "wide",
        // because each pixel takes up 2 bits. Our tile sheet is 16 tiles wide,
        // hence the y * (16*16).
        let offset = tile_y * (16 * 16) + tile_x * 16;

        // Each tile is 8x8 pixels
        for row in 0..8 {
          // A full pattern table is 4KB, or 0x1000

          // Least-significant bit starts at our offset + 0 bytes:
          let mut tile_lsb = self.ppu_read(table_number * 0x1000 + offset + row + 0);
          // Least-significant bit starts at our offset + 8 bytes; one byte per
          // row of pixels in the tile, to skip over the LSB plane:
          let mut tile_msb = self.ppu_read(table_number * 0x1000 + offset + row + 8);

          for col in 0..8 {
            // A 2 bit number; 0, 1, or 2
            //
            // To compute this, we can actually just add these two bits
            // together, since the highest the value can be is 2.
            let pixel_color_index = (tile_lsb & 0x01) + (tile_msb & 0x01);
            let color = self.get_color_from_palette_ram(palette, pixel_color_index);

            // Our pixels are laid out right-to-left in terms of
            // bit-significance, so we _subtract_ our col number from the
            // right-most edge of our tile:
            let pixel_x = (tile_x * 8) + (7 - col);
            let pixel_y = (tile_y * 8) + row;
            let pixel_idx = (pixel_y * 128 + pixel_x) as usize;
            result[pixel_idx][0] = color.r;
            result[pixel_idx][1] = color.g;
            result[pixel_idx][2] = color.b;

            // For our next column, we just need to look at the _next_ bit in
            // our least/most significant bytes. To achieve this, all we need to
            // do is shift them right one bit:
            tile_lsb >>= 1;
            tile_msb >>= 1;
          }
        }
      }
    }

    result
  }

  pub fn get_color_from_palette_ram(&mut self, palette: u8, pixel: u8) -> Color {
    let idx = self.ppu_read(0x3F00 as u16 + ((palette << 2) + pixel) as u16);
    self.ppu.palette.colors[idx as usize]
  }

  pub fn get_palettes(&mut self) -> [[[u8; 4]; 4]; 8] {
    let mut result = [[[0x00, 0x00, 0x00, 0xFF]; 4]; 8];

    for palette_num in 0..8 {
      for color_num in 0..4 {
        let color = self.get_color_from_palette_ram(palette_num, color_num);
        result[palette_num as usize][color_num as usize][0] = color.r;
        result[palette_num as usize][color_num as usize][1] = color.g;
        result[palette_num as usize][color_num as usize][2] = color.b;
        result[palette_num as usize][color_num as usize][3] = 0xFF;
      }
    }

    result
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

    // Get a slice of the program that just contains enough data to disassemble
    // the current instruction; to do this we read from the CPU bus at the
    // current program counter for ~8 bytes or so which should be more than
    // enough.
    let output = disassemble(self, self.cpu.pc, 8);
    let disassembled = &output[0];

    let instruction_data = disassembled
      .data
      .iter()
      .map(|byte| format!("{:02X}", byte))
      .collect::<Vec<String>>()
      .join(" ");

    let cpu = &self.cpu;
    format!(
      "{:04X}  {:<8}  {} {:<26}  A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X}",
      self.cpu.pc,
      instruction_data,
      disassembled.instruction_name,
      disassembled.params,
      cpu.a,
      cpu.x,
      cpu.y,
      cpu.status,
      cpu.s
    )
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

  pub fn ppu_read(&mut self, addr: u16) -> u8 {
    (self as &mut dyn Bus<Ppu>).read(addr)
  }

  pub fn ppu_read16(&mut self, addr: u16) -> u16 {
    (self as &mut dyn Bus<Ppu>).read16(addr)
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
      .or(self.cart.cpu_mapper.safe_read(addr))
      .or(self.ram_mirror.safe_read(&self.ram, addr))
    {
      Some(data) => data,
      None => 0x00,
    }
  }

  fn read(&mut self, addr: u16) -> u8 {
    match None // Hehe, using None here just for formatting purposes:
      .or(self.cart.cpu_mapper.read(addr))
      .or(self.ram_mirror.read(&mut self.ram, addr))
      .or(self.ppu_registers_mirror.read(&mut self.ppu, addr))
    {
      Some(data) => data,
      None => 0x00,
    }
  }

  fn write(&mut self, addr: u16, data: u8) {
    None // Hehe, using None here just for formatting purposes:
      .or_else(|| self.cart.cpu_mapper.write(addr, data))
      .or_else(|| self.ram_mirror.write(&mut self.ram, addr, data))
      .or_else(|| self.ppu_registers_mirror.write(&mut self.ppu, addr, data));
  }
}

/// The PPU's Bus
impl Bus<Ppu> for Nes {
  fn safe_read(&self, _: u16) -> u8 {
    todo!()
  }

  fn read(&mut self, addr_: u16) -> u8 {
    let addr = addr_ & 0x3FFF;
    match None // Hehe, using None here just for formatting purposes:
      .or(self.cart.ppu_mapper.read(addr))
      .or(Some(self.ppu.ppu_read(addr)))
    {
      Some(data) => data,
      None => 0x00,
    }
  }

  fn write(&mut self, addr_: u16, data: u8) {
    let addr = addr_ & 0x3FFF;

    None // Hehe, using None here just for formatting purposes:
      .or_else(|| self.cart.ppu_mapper.write(addr, data))
      .or_else(|| Some(self.ppu.ppu_write(addr, data)));
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    cart::{CartPpuMapper, FLAG_HAS_RAM, FLAG_MIRRORING},
    disassemble,
    mapper::MAPPERS,
    palette::Color,
  };

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

    let cart = Cart::new(&cart_data).unwrap();

    Nes {
      tick: 0,
      cpu,
      ppu,
      cart,
      ram_mirror,
      ram,
      ppu_registers_mirror,
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
      "src/test_fixtures/nestest.nes",
      "src/test_fixtures/ntscpalette.pal",
    ) {
      Ok(n) => n,
      Err(msg) => panic!("{}", msg),
    };

    nes.cpu.pc = 0xC000;

    // First few traces:
    let expected_traces = vec![
      "C000  4C F5 C5  JMP $C5F5                       A:00 X:00 Y:00 P:24 SP:FD",
      "C5F5  A2 00     LDX #$00                        A:00 X:00 Y:00 P:24 SP:FD",
      "C5F7  86 00     STX $00 = 00                    A:00 X:00 Y:00 P:26 SP:FD",
      "C5F9  86 10     STX $10 = 00                    A:00 X:00 Y:00 P:26 SP:FD",
      "C5FB  86 11     STX $11 = 00                    A:00 X:00 Y:00 P:26 SP:FD",
      "C5FD  20 2D C7  JSR $C72D                       A:00 X:00 Y:00 P:26 SP:FD",
      "C72D  EA        NOP                             A:00 X:00 Y:00 P:26 SP:FB",
      "C72E  38        SEC                             A:00 X:00 Y:00 P:26 SP:FB",
      "C72F  B0 04     BCS $C735                       A:00 X:00 Y:00 P:27 SP:FB",
      "C735  EA        NOP                             A:00 X:00 Y:00 P:27 SP:FB",
      "C736  18        CLC                             A:00 X:00 Y:00 P:27 SP:FB",
      "C737  B0 03     BCS $C73C                       A:00 X:00 Y:00 P:26 SP:FB",
      "C739  4C 40 C7  JMP $C740                       A:00 X:00 Y:00 P:26 SP:FB",
      "C740  EA        NOP                             A:00 X:00 Y:00 P:26 SP:FB",
      "C741  38        SEC                             A:00 X:00 Y:00 P:26 SP:FB",
      "C742  90 03     BCC $C747                       A:00 X:00 Y:00 P:27 SP:FB",
      "C744  4C 4B C7  JMP $C74B                       A:00 X:00 Y:00 P:27 SP:FB",
      "C74B  EA        NOP                             A:00 X:00 Y:00 P:27 SP:FB",
      "C74C  18        CLC                             A:00 X:00 Y:00 P:27 SP:FB",
      "C74D  90 04     BCC $C753                       A:00 X:00 Y:00 P:26 SP:FB",
      "C753  EA        NOP                             A:00 X:00 Y:00 P:26 SP:FB",
      "C754  A9 00     LDA #$00                        A:00 X:00 Y:00 P:26 SP:FB",
      "C756  F0 04     BEQ $C75C                       A:00 X:00 Y:00 P:26 SP:FB",
      "C75C  EA        NOP                             A:00 X:00 Y:00 P:26 SP:FB",
      "C75D  A9 40     LDA #$40                        A:00 X:00 Y:00 P:26 SP:FB",
      "C75F  F0 03     BEQ $C764                       A:40 X:00 Y:00 P:24 SP:FB",
      "C761  4C 68 C7  JMP $C768                       A:40 X:00 Y:00 P:24 SP:FB",
      "C768  EA        NOP                             A:40 X:00 Y:00 P:24 SP:FB",
      "C769  A9 40     LDA #$40                        A:40 X:00 Y:00 P:24 SP:FB",
      "C76B  D0 04     BNE $C771                       A:40 X:00 Y:00 P:24 SP:FB",
      "C771  EA        NOP                             A:40 X:00 Y:00 P:24 SP:FB",
      "C772  A9 00     LDA #$00                        A:40 X:00 Y:00 P:24 SP:FB",
      "C774  D0 03     BNE $C779                       A:00 X:00 Y:00 P:26 SP:FB",
      "C776  4C 7D C7  JMP $C77D                       A:00 X:00 Y:00 P:26 SP:FB",
      "C77D  EA        NOP                             A:00 X:00 Y:00 P:26 SP:FB",
      "C77E  A9 FF     LDA #$FF                        A:00 X:00 Y:00 P:26 SP:FB",
      "C780  85 01     STA $01 = 00                    A:FF X:00 Y:00 P:A4 SP:FB",
      "C782  24 01     BIT $01 = FF                    A:FF X:00 Y:00 P:A4 SP:FB",
      "C784  70 04     BVS $C78A                       A:FF X:00 Y:00 P:E4 SP:FB",
      "C78A  EA        NOP                             A:FF X:00 Y:00 P:E4 SP:FB",
      "C78B  24 01     BIT $01 = FF                    A:FF X:00 Y:00 P:E4 SP:FB",
      "C78D  50 03     BVC $C792                       A:FF X:00 Y:00 P:E4 SP:FB",
      "C78F  4C 96 C7  JMP $C796                       A:FF X:00 Y:00 P:E4 SP:FB",
      "C796  EA        NOP                             A:FF X:00 Y:00 P:E4 SP:FB",
      "C797  A9 00     LDA #$00                        A:FF X:00 Y:00 P:E4 SP:FB",
      "C799  85 01     STA $01 = FF                    A:00 X:00 Y:00 P:66 SP:FB",
      "C79B  24 01     BIT $01 = 00                    A:00 X:00 Y:00 P:66 SP:FB",
      "C79D  50 04     BVC $C7A3                       A:00 X:00 Y:00 P:26 SP:FB",
      "C7A3  EA        NOP                             A:00 X:00 Y:00 P:26 SP:FB",
      "C7A4  24 01     BIT $01 = 00                    A:00 X:00 Y:00 P:26 SP:FB",
      "C7A6  70 03     BVS $C7AB                       A:00 X:00 Y:00 P:26 SP:FB",
      "C7A8  4C AF C7  JMP $C7AF                       A:00 X:00 Y:00 P:26 SP:FB",
      "C7AF  EA        NOP                             A:00 X:00 Y:00 P:26 SP:FB",
      "C7B0  A9 00     LDA #$00                        A:00 X:00 Y:00 P:26 SP:FB",
      "C7B2  10 04     BPL $C7B8                       A:00 X:00 Y:00 P:26 SP:FB",
      "C7B8  EA        NOP                             A:00 X:00 Y:00 P:26 SP:FB",
      "C7B9  A9 80     LDA #$80                        A:00 X:00 Y:00 P:26 SP:FB",
      "C7BB  10 03     BPL $C7C0                       A:80 X:00 Y:00 P:A4 SP:FB",
      "C7BD  4C D9 C7  JMP $C7D9                       A:80 X:00 Y:00 P:A4 SP:FB",
      "C7D9  EA        NOP                             A:80 X:00 Y:00 P:A4 SP:FB",
      "C7DA  60        RTS                             A:80 X:00 Y:00 P:A4 SP:FB",
      "C600  20 DB C7  JSR $C7DB                       A:80 X:00 Y:00 P:A4 SP:FD",
      "C7DB  EA        NOP                             A:80 X:00 Y:00 P:A4 SP:FB",
      "C7DC  A9 FF     LDA #$FF                        A:80 X:00 Y:00 P:A4 SP:FB",
      "C7DE  85 01     STA $01 = 00                    A:FF X:00 Y:00 P:A4 SP:FB",
      "C7E0  24 01     BIT $01 = FF                    A:FF X:00 Y:00 P:A4 SP:FB",
      "C7E2  A9 00     LDA #$00                        A:FF X:00 Y:00 P:E4 SP:FB",
      "C7E4  38        SEC                             A:00 X:00 Y:00 P:66 SP:FB",
      "C7E5  78        SEI                             A:00 X:00 Y:00 P:67 SP:FB",
      "C7E6  F8        SED                             A:00 X:00 Y:00 P:67 SP:FB",
      "C7E7  08        PHP                             A:00 X:00 Y:00 P:6F SP:FB",
      "C7E8  68        PLA                             A:00 X:00 Y:00 P:6F SP:FA",
      "C7E9  29 EF     AND #$EF                        A:7F X:00 Y:00 P:6D SP:FB",
      "C7EB  C9 6F     CMP #$6F                        A:6F X:00 Y:00 P:6D SP:FB",
      "C7ED  F0 04     BEQ $C7F3                       A:6F X:00 Y:00 P:6F SP:FB",
      "C7F3  EA        NOP                             A:6F X:00 Y:00 P:6F SP:FB",
      "C7F4  A9 40     LDA #$40                        A:6F X:00 Y:00 P:6F SP:FB",
      "C7F6  85 01     STA $01 = FF                    A:40 X:00 Y:00 P:6D SP:FB",
      "C7F8  24 01     BIT $01 = 40                    A:40 X:00 Y:00 P:6D SP:FB",
      "C7FA  D8        CLD                             A:40 X:00 Y:00 P:6D SP:FB",
      "C7FB  A9 10     LDA #$10                        A:40 X:00 Y:00 P:65 SP:FB",
      "C7FD  18        CLC                             A:10 X:00 Y:00 P:65 SP:FB",
      "C7FE  08        PHP                             A:10 X:00 Y:00 P:64 SP:FB",
      "C7FF  68        PLA                             A:10 X:00 Y:00 P:64 SP:FA",
      "C800  29 EF     AND #$EF                        A:74 X:00 Y:00 P:64 SP:FB",
      "C802  C9 64     CMP #$64                        A:64 X:00 Y:00 P:64 SP:FB",
      "C804  F0 04     BEQ $C80A                       A:64 X:00 Y:00 P:67 SP:FB",
      "C80A  EA        NOP                             A:64 X:00 Y:00 P:67 SP:FB",
      "C80B  A9 80     LDA #$80                        A:64 X:00 Y:00 P:67 SP:FB",
      "C80D  85 01     STA $01 = 40                    A:80 X:00 Y:00 P:E5 SP:FB",
      "C80F  24 01     BIT $01 = 80                    A:80 X:00 Y:00 P:E5 SP:FB",
      "C811  F8        SED                             A:80 X:00 Y:00 P:A5 SP:FB",
      "C812  A9 00     LDA #$00                        A:80 X:00 Y:00 P:AD SP:FB",
      "C814  38        SEC                             A:00 X:00 Y:00 P:2F SP:FB",
      "C815  08        PHP                             A:00 X:00 Y:00 P:2F SP:FB",
      "C816  68        PLA                             A:00 X:00 Y:00 P:2F SP:FA",
      "C817  29 EF     AND #$EF                        A:3F X:00 Y:00 P:2D SP:FB",
      "C819  C9 2F     CMP #$2F                        A:2F X:00 Y:00 P:2D SP:FB",
      "C81B  F0 04     BEQ $C821                       A:2F X:00 Y:00 P:2F SP:FB",
      "C821  EA        NOP                             A:2F X:00 Y:00 P:2F SP:FB",
      "C822  A9 FF     LDA #$FF                        A:2F X:00 Y:00 P:2F SP:FB",
      "C824  48        PHA                             A:FF X:00 Y:00 P:AD SP:FB",
      "C825  28        PLP                             A:FF X:00 Y:00 P:AD SP:FA",
      "C826  D0 09     BNE $C831                       A:FF X:00 Y:00 P:EF SP:FB",
      "C828  10 07     BPL $C831                       A:FF X:00 Y:00 P:EF SP:FB",
      "C82A  50 05     BVC $C831                       A:FF X:00 Y:00 P:EF SP:FB",
      "C82C  90 03     BCC $C831                       A:FF X:00 Y:00 P:EF SP:FB",
      "C82E  4C 35 C8  JMP $C835                       A:FF X:00 Y:00 P:EF SP:FB",
      "C835  EA        NOP                             A:FF X:00 Y:00 P:EF SP:FB",
      "C836  A9 04     LDA #$04                        A:FF X:00 Y:00 P:EF SP:FB",
      "C838  48        PHA                             A:04 X:00 Y:00 P:6D SP:FB",
      "C839  28        PLP                             A:04 X:00 Y:00 P:6D SP:FA",
      "C83A  F0 09     BEQ $C845                       A:04 X:00 Y:00 P:24 SP:FB",
      "C83C  30 07     BMI $C845                       A:04 X:00 Y:00 P:24 SP:FB",
      "C83E  70 05     BVS $C845                       A:04 X:00 Y:00 P:24 SP:FB",
      "C840  B0 03     BCS $C845                       A:04 X:00 Y:00 P:24 SP:FB",
      "C842  4C 49 C8  JMP $C849                       A:04 X:00 Y:00 P:24 SP:FB",
      "C849  EA        NOP                             A:04 X:00 Y:00 P:24 SP:FB",
      "C84A  F8        SED                             A:04 X:00 Y:00 P:24 SP:FB",
      "C84B  A9 FF     LDA #$FF                        A:04 X:00 Y:00 P:2C SP:FB",
      "C84D  85 01     STA $01 = 80                    A:FF X:00 Y:00 P:AC SP:FB",
      "C84F  24 01     BIT $01 = FF                    A:FF X:00 Y:00 P:AC SP:FB",
      "C851  18        CLC                             A:FF X:00 Y:00 P:EC SP:FB",
      "C852  A9 00     LDA #$00                        A:FF X:00 Y:00 P:EC SP:FB",
      "C854  48        PHA                             A:00 X:00 Y:00 P:6E SP:FB",
      "C855  A9 FF     LDA #$FF                        A:00 X:00 Y:00 P:6E SP:FA",
      "C857  68        PLA                             A:FF X:00 Y:00 P:EC SP:FA",
      "C858  D0 09     BNE $C863                       A:00 X:00 Y:00 P:6E SP:FB",
      "C85A  30 07     BMI $C863                       A:00 X:00 Y:00 P:6E SP:FB",
      "C85C  50 05     BVC $C863                       A:00 X:00 Y:00 P:6E SP:FB",
      "C85E  B0 03     BCS $C863                       A:00 X:00 Y:00 P:6E SP:FB",
      "C860  4C 67 C8  JMP $C867                       A:00 X:00 Y:00 P:6E SP:FB",
      "C867  EA        NOP                             A:00 X:00 Y:00 P:6E SP:FB",
      "C868  A9 00     LDA #$00                        A:00 X:00 Y:00 P:6E SP:FB",
      "C86A  85 01     STA $01 = FF                    A:00 X:00 Y:00 P:6E SP:FB",
      "C86C  24 01     BIT $01 = 00                    A:00 X:00 Y:00 P:6E SP:FB",
      "C86E  38        SEC                             A:00 X:00 Y:00 P:2E SP:FB",
      "C86F  A9 FF     LDA #$FF                        A:00 X:00 Y:00 P:2F SP:FB",
      "C871  48        PHA                             A:FF X:00 Y:00 P:AD SP:FB",
      "C872  A9 00     LDA #$00                        A:FF X:00 Y:00 P:AD SP:FA",
      "C874  68        PLA                             A:00 X:00 Y:00 P:2F SP:FA",
      "C875  F0 09     BEQ $C880                       A:FF X:00 Y:00 P:AD SP:FB",
      "C877  10 07     BPL $C880                       A:FF X:00 Y:00 P:AD SP:FB",
      "C879  70 05     BVS $C880                       A:FF X:00 Y:00 P:AD SP:FB",
      "C87B  90 03     BCC $C880                       A:FF X:00 Y:00 P:AD SP:FB",
      "C87D  4C 84 C8  JMP $C884                       A:FF X:00 Y:00 P:AD SP:FB",
      "C884  60        RTS                             A:FF X:00 Y:00 P:AD SP:FB",
      "C603  20 85 C8  JSR $C885                       A:FF X:00 Y:00 P:AD SP:FD",
      "C885  EA        NOP                             A:FF X:00 Y:00 P:AD SP:FB",
      "C886  18        CLC                             A:FF X:00 Y:00 P:AD SP:FB",
      "C887  A9 FF     LDA #$FF                        A:FF X:00 Y:00 P:AC SP:FB",
      "C889  85 01     STA $01 = 00                    A:FF X:00 Y:00 P:AC SP:FB",
      "C88B  24 01     BIT $01 = FF                    A:FF X:00 Y:00 P:AC SP:FB",
      "C88D  A9 55     LDA #$55                        A:FF X:00 Y:00 P:EC SP:FB",
      "C88F  09 AA     ORA #$AA                        A:55 X:00 Y:00 P:6C SP:FB",
      "C891  B0 0B     BCS $C89E                       A:FF X:00 Y:00 P:EC SP:FB",
      "C893  10 09     BPL $C89E                       A:FF X:00 Y:00 P:EC SP:FB",
      "C895  C9 FF     CMP #$FF                        A:FF X:00 Y:00 P:EC SP:FB",
      "C897  D0 05     BNE $C89E                       A:FF X:00 Y:00 P:6F SP:FB",
      "C899  50 03     BVC $C89E                       A:FF X:00 Y:00 P:6F SP:FB",
      "C89B  4C A2 C8  JMP $C8A2                       A:FF X:00 Y:00 P:6F SP:FB",
      "C8A2  EA        NOP                             A:FF X:00 Y:00 P:6F SP:FB",
      "C8A3  38        SEC                             A:FF X:00 Y:00 P:6F SP:FB",
      "C8A4  B8        CLV                             A:FF X:00 Y:00 P:6F SP:FB",
      "C8A5  A9 00     LDA #$00                        A:FF X:00 Y:00 P:2F SP:FB",
      "C8A7  09 00     ORA #$00                        A:00 X:00 Y:00 P:2F SP:FB",
      "C8A9  D0 09     BNE $C8B4                       A:00 X:00 Y:00 P:2F SP:FB",
      "C8AB  70 07     BVS $C8B4                       A:00 X:00 Y:00 P:2F SP:FB",
      "C8AD  90 05     BCC $C8B4                       A:00 X:00 Y:00 P:2F SP:FB",
      "C8AF  30 03     BMI $C8B4                       A:00 X:00 Y:00 P:2F SP:FB",
      "C8B1  4C B8 C8  JMP $C8B8                       A:00 X:00 Y:00 P:2F SP:FB",
      "C8B8  EA        NOP                             A:00 X:00 Y:00 P:2F SP:FB",
      "C8B9  18        CLC                             A:00 X:00 Y:00 P:2F SP:FB",
      "C8BA  24 01     BIT $01 = FF                    A:00 X:00 Y:00 P:2E SP:FB",
      "C8BC  A9 55     LDA #$55                        A:00 X:00 Y:00 P:EE SP:FB",
      "C8BE  29 AA     AND #$AA                        A:55 X:00 Y:00 P:6C SP:FB",
      "C8C0  D0 09     BNE $C8CB                       A:00 X:00 Y:00 P:6E SP:FB",
      "C8C2  50 07     BVC $C8CB                       A:00 X:00 Y:00 P:6E SP:FB",
      "C8C4  B0 05     BCS $C8CB                       A:00 X:00 Y:00 P:6E SP:FB",
      "C8C6  30 03     BMI $C8CB                       A:00 X:00 Y:00 P:6E SP:FB",
      "C8C8  4C CF C8  JMP $C8CF                       A:00 X:00 Y:00 P:6E SP:FB",
      "C8CF  EA        NOP                             A:00 X:00 Y:00 P:6E SP:FB",
      "C8D0  38        SEC                             A:00 X:00 Y:00 P:6E SP:FB",
      "C8D1  B8        CLV                             A:00 X:00 Y:00 P:6F SP:FB",
      "C8D2  A9 F8     LDA #$F8                        A:00 X:00 Y:00 P:2F SP:FB",
      "C8D4  29 EF     AND #$EF                        A:F8 X:00 Y:00 P:AD SP:FB",
      "C8D6  90 0B     BCC $C8E3                       A:E8 X:00 Y:00 P:AD SP:FB",
      "C8D8  10 09     BPL $C8E3                       A:E8 X:00 Y:00 P:AD SP:FB",
      "C8DA  C9 E8     CMP #$E8                        A:E8 X:00 Y:00 P:AD SP:FB",
      "C8DC  D0 05     BNE $C8E3                       A:E8 X:00 Y:00 P:2F SP:FB",
      "C8DE  70 03     BVS $C8E3                       A:E8 X:00 Y:00 P:2F SP:FB",
      "C8E0  4C E7 C8  JMP $C8E7                       A:E8 X:00 Y:00 P:2F SP:FB",
      "C8E7  EA        NOP                             A:E8 X:00 Y:00 P:2F SP:FB",
      "C8E8  18        CLC                             A:E8 X:00 Y:00 P:2F SP:FB",
      "C8E9  24 01     BIT $01 = FF                    A:E8 X:00 Y:00 P:2E SP:FB",
      "C8EB  A9 5F     LDA #$5F                        A:E8 X:00 Y:00 P:EC SP:FB",
      "C8ED  49 AA     EOR #$AA                        A:5F X:00 Y:00 P:6C SP:FB",
      "C8EF  B0 0B     BCS $C8FC                       A:F5 X:00 Y:00 P:EC SP:FB",
      "C8F1  10 09     BPL $C8FC                       A:F5 X:00 Y:00 P:EC SP:FB",
      "C8F3  C9 F5     CMP #$F5                        A:F5 X:00 Y:00 P:EC SP:FB",
      "C8F5  D0 05     BNE $C8FC                       A:F5 X:00 Y:00 P:6F SP:FB",
      "C8F7  50 03     BVC $C8FC                       A:F5 X:00 Y:00 P:6F SP:FB",
      "C8F9  4C 00 C9  JMP $C900                       A:F5 X:00 Y:00 P:6F SP:FB",
      "C900  EA        NOP                             A:F5 X:00 Y:00 P:6F SP:FB",
      "C901  38        SEC                             A:F5 X:00 Y:00 P:6F SP:FB",
      "C902  B8        CLV                             A:F5 X:00 Y:00 P:6F SP:FB",
      "C903  A9 70     LDA #$70                        A:F5 X:00 Y:00 P:2F SP:FB",
      "C905  49 70     EOR #$70                        A:70 X:00 Y:00 P:2D SP:FB",
      "C907  D0 09     BNE $C912                       A:00 X:00 Y:00 P:2F SP:FB",
      "C909  70 07     BVS $C912                       A:00 X:00 Y:00 P:2F SP:FB",
      "C90B  90 05     BCC $C912                       A:00 X:00 Y:00 P:2F SP:FB",
      "C90D  30 03     BMI $C912                       A:00 X:00 Y:00 P:2F SP:FB",
      "C90F  4C 16 C9  JMP $C916                       A:00 X:00 Y:00 P:2F SP:FB",
      "C916  EA        NOP                             A:00 X:00 Y:00 P:2F SP:FB",
      "C917  18        CLC                             A:00 X:00 Y:00 P:2F SP:FB",
      "C918  24 01     BIT $01 = FF                    A:00 X:00 Y:00 P:2E SP:FB",
      "C91A  A9 00     LDA #$00                        A:00 X:00 Y:00 P:EE SP:FB",
      "C91C  69 69     ADC #$69                        A:00 X:00 Y:00 P:6E SP:FB",
      "C91E  30 0B     BMI $C92B                       A:69 X:00 Y:00 P:2C SP:FB",
      "C920  B0 09     BCS $C92B                       A:69 X:00 Y:00 P:2C SP:FB",
      "C922  C9 69     CMP #$69                        A:69 X:00 Y:00 P:2C SP:FB",
      "C924  D0 05     BNE $C92B                       A:69 X:00 Y:00 P:2F SP:FB",
      "C926  70 03     BVS $C92B                       A:69 X:00 Y:00 P:2F SP:FB",
      "C928  4C 2F C9  JMP $C92F                       A:69 X:00 Y:00 P:2F SP:FB",
      "C92F  EA        NOP                             A:69 X:00 Y:00 P:2F SP:FB",
      "C930  38        SEC                             A:69 X:00 Y:00 P:2F SP:FB",
      "C931  F8        SED                             A:69 X:00 Y:00 P:2F SP:FB",
      "C932  24 01     BIT $01 = FF                    A:69 X:00 Y:00 P:2F SP:FB",
      "C934  A9 01     LDA #$01                        A:69 X:00 Y:00 P:ED SP:FB",
      "C936  69 69     ADC #$69                        A:01 X:00 Y:00 P:6D SP:FB",
      "C938  30 0B     BMI $C945                       A:6B X:00 Y:00 P:2C SP:FB",
      "C93A  B0 09     BCS $C945                       A:6B X:00 Y:00 P:2C SP:FB",
      "C93C  C9 6B     CMP #$6B                        A:6B X:00 Y:00 P:2C SP:FB",
      "C93E  D0 05     BNE $C945                       A:6B X:00 Y:00 P:2F SP:FB",
      "C940  70 03     BVS $C945                       A:6B X:00 Y:00 P:2F SP:FB",
      "C942  4C 49 C9  JMP $C949                       A:6B X:00 Y:00 P:2F SP:FB",
      "C949  EA        NOP                             A:6B X:00 Y:00 P:2F SP:FB",
      "C94A  D8        CLD                             A:6B X:00 Y:00 P:2F SP:FB",
      "C94B  38        SEC                             A:6B X:00 Y:00 P:27 SP:FB",
      "C94C  B8        CLV                             A:6B X:00 Y:00 P:27 SP:FB",
      "C94D  A9 7F     LDA #$7F                        A:6B X:00 Y:00 P:27 SP:FB",
      "C94F  69 7F     ADC #$7F                        A:7F X:00 Y:00 P:25 SP:FB",
      "C951  10 0B     BPL $C95E                       A:FF X:00 Y:00 P:E4 SP:FB",
      "C953  B0 09     BCS $C95E                       A:FF X:00 Y:00 P:E4 SP:FB",
      "C955  C9 FF     CMP #$FF                        A:FF X:00 Y:00 P:E4 SP:FB",
      "C957  D0 05     BNE $C95E                       A:FF X:00 Y:00 P:67 SP:FB",
      "C959  50 03     BVC $C95E                       A:FF X:00 Y:00 P:67 SP:FB",
      "C95B  4C 62 C9  JMP $C962                       A:FF X:00 Y:00 P:67 SP:FB",
      "C962  EA        NOP                             A:FF X:00 Y:00 P:67 SP:FB",
      "C963  18        CLC                             A:FF X:00 Y:00 P:67 SP:FB",
      "C964  24 01     BIT $01 = FF                    A:FF X:00 Y:00 P:66 SP:FB",
      "C966  A9 7F     LDA #$7F                        A:FF X:00 Y:00 P:E4 SP:FB",
      "C968  69 80     ADC #$80                        A:7F X:00 Y:00 P:64 SP:FB",
      "C96A  10 0B     BPL $C977                       A:FF X:00 Y:00 P:A4 SP:FB",
      "C96C  B0 09     BCS $C977                       A:FF X:00 Y:00 P:A4 SP:FB",
      "C96E  C9 FF     CMP #$FF                        A:FF X:00 Y:00 P:A4 SP:FB",
      "C970  D0 05     BNE $C977                       A:FF X:00 Y:00 P:27 SP:FB",
      "C972  70 03     BVS $C977                       A:FF X:00 Y:00 P:27 SP:FB",
      "C974  4C 7B C9  JMP $C97B                       A:FF X:00 Y:00 P:27 SP:FB",
      "C97B  EA        NOP                             A:FF X:00 Y:00 P:27 SP:FB",
      "C97C  38        SEC                             A:FF X:00 Y:00 P:27 SP:FB",
      "C97D  B8        CLV                             A:FF X:00 Y:00 P:27 SP:FB",
      "C97E  A9 7F     LDA #$7F                        A:FF X:00 Y:00 P:27 SP:FB",
      "C980  69 80     ADC #$80                        A:7F X:00 Y:00 P:25 SP:FB",
      "C982  D0 09     BNE $C98D                       A:00 X:00 Y:00 P:27 SP:FB",
      "C984  30 07     BMI $C98D                       A:00 X:00 Y:00 P:27 SP:FB",
    ];

    for expected_trace in expected_traces {
      assert_eq!(nes.trace(), expected_trace);
      nes.step();
    }
  }
}
