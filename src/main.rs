#[macro_use]
extern crate maplit;

use docopt::Docopt;
use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use ppu::{SCREEN_H, SCREEN_W};
use serde::Deserialize;
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

mod apu;
mod audio;
mod bus;
mod bus_device;
mod cart;
mod cpu6502;
mod disassemble;
mod gui;
mod mapper;
mod mirror;
mod nes;
mod palette;
mod peripherals;
mod ppu;
mod ram;
mod trace;

use crate::gui::Gui;
use crate::nes::Nes;

const USAGE: &'static str = "
Usage:

nessers <rom> [<breakpoints>]
";

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;

#[derive(Deserialize)]
struct Args {
  arg_rom: String,
  arg_breakpoints: Vec<String>,
}

fn main() -> Result<(), Error> {
  env_logger::init();
  let event_loop = EventLoop::new();
  let mut input = WinitInputHelper::new();
  let window = {
    let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
    WindowBuilder::new()
      .with_title("nessers")
      .with_inner_size(size)
      .with_min_inner_size(size)
      .build(&event_loop)
      .unwrap()
  };

  let mut scale_factor = window.scale_factor();

  let mut pixels = {
    let window_size = window.inner_size();
    let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
    Pixels::new(WIDTH, HEIGHT, surface_texture)?
  };

  let args: Args = Docopt::new(USAGE)
    .and_then(|d| d.deserialize())
    .unwrap_or_else(|e| e.exit());

  let mut nes = match Nes::new(&args.arg_rom, "src/test_fixtures/ntscpalette.pal") {
    Ok(n) => n,
    Err(msg) => panic!("{}", msg),
  };

  nes.reset();
  nes.step();

  let mut nes_debugger = NesDebugger::new(WIDTH, HEIGHT, nes);

  // Set up Dear ImGui
  let mut gui = Gui::new(&window, &pixels);

  event_loop.run(move |event, _, control_flow| {
    // Draw the current frame
    if let Event::RedrawRequested(_) = event {
      // Draw the world
      nes_debugger.draw(pixels.get_frame());

      // Prepare Dear ImGui
      gui.prepare(&window).expect("gui.prepare() failed");

      // Render everything together
      let render_result = pixels.render_with(|encoder, render_target, context| {
        // Render the world texture
        context.scaling_renderer.render(encoder, render_target);

        // Render Dear ImGui
        gui.render(&window, encoder, render_target, context)?;

        Ok(())
      });

      // Basic error handling
      if render_result
        .map_err(|e| error!("pixels.render() failed: {}", e))
        .is_err()
      {
        *control_flow = ControlFlow::Exit;
        return;
      }
    }

    // Handle input events
    gui.handle_event(&window, &event);
    if input.update(&event) {
      // Close events
      if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
        *control_flow = ControlFlow::Exit;
        return;
      }

      // Player 1 controls
      nes_debugger.nes.peripherals.controllers[0].a = input.key_held(VirtualKeyCode::X);
      nes_debugger.nes.peripherals.controllers[0].b = input.key_held(VirtualKeyCode::Z);
      nes_debugger.nes.peripherals.controllers[0].select = input.key_held(VirtualKeyCode::RShift);
      nes_debugger.nes.peripherals.controllers[0].start = input.key_held(VirtualKeyCode::Return);
      nes_debugger.nes.peripherals.controllers[0].up = input.key_held(VirtualKeyCode::Up);
      nes_debugger.nes.peripherals.controllers[0].down = input.key_held(VirtualKeyCode::Down);
      nes_debugger.nes.peripherals.controllers[0].left = input.key_held(VirtualKeyCode::Left);
      nes_debugger.nes.peripherals.controllers[0].right = input.key_held(VirtualKeyCode::Right);

      // Update the scale factor
      if let Some(factor) = input.scale_factor() {
        scale_factor = factor;
      }

      // Resize the window
      if let Some(size) = input.window_resized() {
        if size.width > 0 && size.height > 0 {
          // Resize the surface texture
          pixels.resize_surface(size.width, size.height);

          // Resize the world
          let LogicalSize { width, height } = size.to_logical(scale_factor);
          nes_debugger.resize(width, height);
          pixels.resize_buffer(width, height);
        }
      }

      // Update internal state and request a redraw
      nes_debugger.update();
      window.request_redraw();
    }
  });
}

/// Tying it all together.
struct NesDebugger {
  width: i16,
  height: i16,
  nes: Nes,
  dirty: bool,
  odd: bool,
}

impl NesDebugger {
  /// Create a new `World` instance that can draw a moving box.
  pub fn new(width: u32, height: u32, nes: Nes) -> Self {
    Self {
      width: width as i16,
      height: height as i16,
      nes,
      dirty: false,
      odd: false,
    }
  }

  /// Update the `World` internal state; bounce the box around the screen.
  pub fn update(&mut self) {}

  /// Resize the world
  pub fn resize(&mut self, width: u32, height: u32) {
    self.width = width as i16;
    self.height = height as i16;
    self.dirty = true;
  }

  /// Draw the `World` state to the frame buffer.
  ///
  /// Assumes the default texture format: `wgpu::TextureFormat::Rgba8UnormSrgb`
  pub fn draw(&mut self, frame: &mut [u8]) {
    // My display is 120hz so I'm just brute forcing this for now lol:
    if self.odd {
      self.nes.frame();
    }
    self.odd = !self.odd;

    for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
      let x = (i % self.width as usize) / 2;
      let y = (i / self.width as usize) / 2;
      if x < SCREEN_W && y > 16 && y < (SCREEN_H + 16) {
        let ppu_screen_idx = (y - 16) * SCREEN_W + x;
        pixel.copy_from_slice(&self.nes.ppu.screen[ppu_screen_idx]);
      } else if self.dirty {
        pixel.copy_from_slice(&[0x00, 0x00, 0x00, 0xFF]);
      }
    }
  }
}
