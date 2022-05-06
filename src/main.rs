#[macro_use]
extern crate maplit;

use coffee::graphics::{Color, Frame, Gpu, Window, WindowSettings};
use coffee::input::{self, keyboard, Input};
use coffee::load::Task;
use coffee::ui::{Align, Column, Element, Image, Justify, Renderer, Row, Text, UserInterface};
use coffee::{Game, Result, Timer};
use std::collections::HashSet;

pub mod bus;
pub mod bus_device;
pub mod cart;
pub mod cpu6502;
pub mod disassemble;
pub mod mapper;
pub mod mirror;
pub mod nes;
pub mod palette;
pub mod ppu;
pub mod ram;

use crate::cpu6502::{StatusFlag, PC_INIT_ADDR, STACK_SIZE};
use crate::disassemble::disassemble;
use crate::nes::Nes;
use crate::ppu::{SCREEN_H, SCREEN_W};

fn main() -> Result<()> {
  <NESDebugger as UserInterface>::run(WindowSettings {
    title: String::from("nessers"),
    size: (1920, 1080),
    resizable: false,
    fullscreen: false,
    maximized: false,
  })
}

struct NESDebugger {
  screen_img: Option<coffee::graphics::Image>,
  pattern_table_0_img: Option<coffee::graphics::Image>,
  pattern_table_1_img: Option<coffee::graphics::Image>,
  palettes_imgs: Option<[coffee::graphics::Image; 8]>,
  nes: Nes,
}

// Fibonacci sequence program:
// let mut debugger_ui = NESDebugger {
//     screen: [0x00; SCREEN_W * SCREEN_H],
//     cpu: Processor::new(Bus::new(vec![Box::new(Ram::new(0x0000, 0xFFFF + 1))])),
// };

// let program_start: u16 = 0x8000;

// debugger_ui.nes.cpu.bus.write16(PC_INIT_ADDR, program_start);

// let program: Vec<u8> = vec![
//     // Initialize A to 0
//     0xA9, 0, // LDA #0
//     // Set X to 0x0000 + 0
//     0xA2, 0, // LDX #0
//     // [0, ...]
//     //  ^
//     0x95, 0x00, // STA #0
//     // Set A to 1
//     0xA9, 1, // LDA #1
//     // [0, 1, ...]
//     //     ^
//     0x95, 0x01, // STA #1
//     //
//     // LOOP:
//     //
//     // A = A + RAM[X]
//     0x75, 0x00, // ADC $00,X
//     // RAM[X + 2] = A
//     0x95, 0x02, // STA $02,X
//     // Increment X
//     0xE8, // INX
//     // JMP Loop
//     0x4C, 10, 0x80,
// ];
// let mut offset: u16 = 0;
// for byte in &program {
//     debugger_ui.nes.cpu.bus.write(program_start + offset, *byte);
//     offset += 1;
// }

// debugger_ui.nes.cpu.sig_reset();
// debugger_ui.nes.cpu.step();

impl Game for NESDebugger {
  type Input = CPUDebuggerInput; // No input data
  type LoadingScreen = (); // No loading screen

  fn load(_window: &Window) -> Task<NESDebugger> {
    // Load your game assets here. Check out the `load` module!
    Task::succeed(|| {
      let nes = match Nes::new(
        "src/test_fixtures/nestest.nes",
        "src/test_fixtures/ntscpalette.pal",
      ) {
        Ok(n) => n,
        Err(msg) => panic!("{}", msg),
      };

      let mut debugger_ui = NESDebugger {
        screen_img: None,
        pattern_table_0_img: None,
        pattern_table_1_img: None,
        palettes_imgs: None,
        nes,
      };
      debugger_ui.nes.reset();
      debugger_ui.nes.step();

      debugger_ui
    })
  }

  fn draw(&mut self, frame: &mut Frame, _timer: &Timer) {
    // Clear the current frame
    frame.clear(Color {
      r: 0.3,
      g: 0.3,
      b: 0.6,
      a: 1.0,
    });
  }

  fn interact(&mut self, input: &mut CPUDebuggerInput, window: &mut Window) {
    for keypress in &input.keypresses {
      let key = format!("{:?}", keypress);
      if key == "Space" {
        self.nes.step();
      } else if key == "R" {
        self.nes.reset();
      } else if key == "F" {
        self.nes.frame();
      }
    }

    input.keypresses.clear();

    // Update the screen image:
    self.screen_img = Some(from_screen(window.gpu(), &self.nes.ppu.screen).unwrap());

    // Get the pattern table image:
    self.pattern_table_0_img =
      Some(from_pattern_table(window.gpu(), &self.nes.render_pattern_table(0, 0)).unwrap());
    self.pattern_table_1_img =
      Some(from_pattern_table(window.gpu(), &self.nes.render_pattern_table(1, 0)).unwrap());

    // lol
    let palettes = self.nes.get_palettes();
    self.palettes_imgs = Some([
      from_palette(window.gpu(), &palettes[0]).unwrap(),
      from_palette(window.gpu(), &palettes[1]).unwrap(),
      from_palette(window.gpu(), &palettes[2]).unwrap(),
      from_palette(window.gpu(), &palettes[3]).unwrap(),
      from_palette(window.gpu(), &palettes[4]).unwrap(),
      from_palette(window.gpu(), &palettes[5]).unwrap(),
      from_palette(window.gpu(), &palettes[6]).unwrap(),
      from_palette(window.gpu(), &palettes[7]).unwrap(),
    ]);
  }
}

enum Message {}

const ACTIVE_COLOR: Color = Color {
  r: 1.0,
  g: 1.0,
  b: 1.0,
  a: 1.0,
};

const INACTIVE_COLOR: Color = Color {
  r: 1.0,
  g: 1.0,
  b: 1.0,
  a: 0.5,
};

impl UserInterface for NESDebugger {
  type Message = Message;
  type Renderer = Renderer;
  fn react(&mut self, _event: Message, _window: &mut Window) {
    // Does nothing
  }
  fn layout(&mut self, window: &Window) -> Element<Message> {
    let mut stack_str = String::new();
    for page in 0..(STACK_SIZE / 16) {
      let addr = 0 as u16 + (page as u16) * 16;
      stack_str.push_str(&format!("{:04X}: ", addr));
      for offset in 0..16 {
        stack_str.push_str(&format!("{:02X} ", self.nes.cpu_read(addr + offset)));
      }
      stack_str.push_str("\n");
    }

    let first_pc_page = (self.nes.cpu.pc / 16) as u16;
    let mut ram_str = String::new();
    for page in 0..(STACK_SIZE / 16) {
      let addr = ((first_pc_page + page as u16) as u16) * 16;
      ram_str.push_str(&format!("{:04X}: ", addr));
      for offset in 0..16 {
        let addr = addr + offset;
        if addr == self.nes.cpu.pc {
          ram_str.push_str(&format!("{:02X}<", self.nes.cpu_read(addr)));
        } else {
          ram_str.push_str(&format!("{:02X} ", self.nes.cpu_read(addr)));
        }
      }
      ram_str.push_str("\n");
    }

    let left_pane = Column::new()
      .push(Text::new("---").size(30))
      .push(Text::new(&stack_str).size(30))
      .push(Text::new("---").size(30))
      .push(Text::new(&ram_str).size(30));

    let mut program: Vec<u8> = vec![];
    let program_start = self.nes.cpu_read16(PC_INIT_ADDR);
    let mut pc = program_start;
    while pc < self.nes.cpu.pc + 128 {
      program.push(self.nes.cpu_read(pc));
      pc += 1;
    }
    let disassembled = disassemble(&program);
    let mut disassembled_output: Vec<String> = vec![];
    let mut pc_idx: i32 = 0;
    let mut idx: i32 = 0;
    for o in disassembled {
      let current = self.nes.cpu.pc == program_start + o.offset;
      if current {
        pc_idx = idx;
      }
      disassembled_output.push(format!(
        "{} ${:04X}: {} {}",
        if current { ">" } else { " " },
        program_start + o.offset,
        o.instruction_name,
        o.params
      ));
      idx += 1;
    }
    let start = (pc_idx - 8).max(0).min(disassembled_output.len() as i32) as usize;
    let end = ((start as i32) + 32)
      .max(0)
      .min(disassembled_output.len() as i32) as usize;
    let disassembled_output = &disassembled_output[start..end];

    let center_pane = Column::new()
      .width(400)
      .push(
        Row::new()
          .push(Text::new("Status:").size(30))
          .push(Text::new("C").size(30).color(
            if self.nes.cpu.get_status(StatusFlag::Carry) != 0x00 {
              ACTIVE_COLOR
            } else {
              INACTIVE_COLOR
            },
          ))
          .push(Text::new("Z").size(30).color(
            if self.nes.cpu.get_status(StatusFlag::Zero) != 0x00 {
              ACTIVE_COLOR
            } else {
              INACTIVE_COLOR
            },
          ))
          .push(Text::new("I").size(30).color(
            if self.nes.cpu.get_status(StatusFlag::DisableInterrupts) != 0x00 {
              ACTIVE_COLOR
            } else {
              INACTIVE_COLOR
            },
          ))
          .push(Text::new("B").size(30).color(
            if self.nes.cpu.get_status(StatusFlag::Break) != 0x00 {
              ACTIVE_COLOR
            } else {
              INACTIVE_COLOR
            },
          ))
          .push(Text::new("O").size(30).color(
            if self.nes.cpu.get_status(StatusFlag::Overflow) != 0x00 {
              ACTIVE_COLOR
            } else {
              INACTIVE_COLOR
            },
          ))
          .push(Text::new("N").size(30).color(
            if self.nes.cpu.get_status(StatusFlag::Negative) != 0x00 {
              ACTIVE_COLOR
            } else {
              INACTIVE_COLOR
            },
          )),
      )
      .push(Text::new(&format!("PC: {:04X} -", self.nes.cpu.pc)).size(30))
      .push(Text::new(&format!(" A: {:02X} ({})", self.nes.cpu.a, self.nes.cpu.a)).size(30))
      .push(Text::new(&format!(" X: {:02X} ({})", self.nes.cpu.x, self.nes.cpu.x)).size(30))
      .push(Text::new(&format!(" Y: {:02X} ({})", self.nes.cpu.y, self.nes.cpu.y)).size(30))
      .push(Text::new("---".into()).size(30))
      .push(Text::new(&disassembled_output.join("\n")).size(30));

    let mut ui = Row::new()
      .padding(16)
      .spacing(16)
      .width(window.width() as u32)
      .height(window.height() as u32)
      .push(left_pane)
      .push(center_pane);

    ui = match &self.screen_img {
      Some(img) => ui
        // .push(Text::new("Screen:").size(30))
        .push(Image::new(&img)),
      None => ui,
    };

    let mut tables = Column::new().height(window.height() as u32);
    tables = match &self.pattern_table_0_img {
      Some(img) => tables.push(Image::new(&img)),
      None => tables,
    };
    tables = match &self.pattern_table_1_img {
      Some(img) => tables.push(Image::new(&img)),
      None => tables,
    };

    tables = match &self.palettes_imgs {
      Some(imgs) => {
        for i in 0..8 {
          tables = tables.push(Image::new(&imgs[i]));
        }
        tables
      }
      None => tables,
    };

    ui = ui.push(tables);

    ui.into()
  }
}

struct CPUDebuggerInput {
  keypresses: HashSet<keyboard::KeyCode>,
}

impl Input for CPUDebuggerInput {
  fn new() -> Self {
    CPUDebuggerInput {
      keypresses: HashSet::new(),
    }
  }
  fn update(&mut self, event: input::Event) {
    match event {
      input::Event::Keyboard(keyboard::Event::Input {
        key_code,
        state: input::ButtonState::Pressed,
      }) => {
        println!("{:?}", key_code);
        self.keypresses.insert(key_code);
      }
      _ => {}
    }
  }
  fn clear(&mut self) {}
}

fn from_screen(
  gpu: &mut Gpu,
  screen: &[[u8; 4]; SCREEN_W * SCREEN_H],
) -> Result<coffee::graphics::Image> {
  let colors: Vec<[u8; 4]> = screen
    .iter()
    // For now, we just plop the pixel
    .map(|color| *color)
    .collect();

  coffee::graphics::Image::from_image(
    gpu,
    &image::DynamicImage::ImageRgba8(
      image::RgbaImage::from_raw(
        SCREEN_W as u32,
        SCREEN_H as u32,
        colors.iter().flatten().cloned().collect(),
      )
      .unwrap(),
    ),
  )
}

fn from_pattern_table(
  gpu: &mut Gpu,
  pattern_table: &[[u8; 4]; 128 * 128],
) -> Result<coffee::graphics::Image> {
  let colors: Vec<[u8; 4]> = pattern_table
    .iter()
    // For now, we just plop the pixel
    .map(|color| *color)
    .collect();

  coffee::graphics::Image::from_image(
    gpu,
    &image::DynamicImage::ImageRgba8(
      image::RgbaImage::from_raw(
        128 as u32,
        128 as u32,
        colors.iter().flatten().cloned().collect(),
      )
      .unwrap(),
    ),
  )
}

fn from_palette(gpu: &mut Gpu, palettes: &[[u8; 4]; 4]) -> Result<coffee::graphics::Image> {
  let colors: Vec<[u8; 4]> = palettes
    .iter()
    // For now, we just plop the pixel
    .map(|color| *color)
    .collect();

  coffee::graphics::Image::from_image(
    gpu,
    &image::DynamicImage::ImageRgba8(
      image::RgbaImage::from_raw(
        4 as u32,
        1 as u32,
        colors.iter().flatten().cloned().collect(),
      )
      .unwrap(),
    ),
  )
}
