#[macro_use]
extern crate maplit;

use std::sync::mpsc;

use audio::AudioDevice;
use cpal::traits::StreamTrait;
use docopt::Docopt;
use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use ppu::{SCREEN_H, SCREEN_W};
use serde::Deserialize;
use std::time::{Duration, Instant};
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

use crate::gui::Framework;
use crate::nes::Nes;

const USAGE: &'static str = "
Usage:

nessers <rom> [<breakpoints>...]
";

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 960;

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

  let (mut pixels, mut framework) = {
    let scale_factor = window.scale_factor() as f32;
    let window_size = window.inner_size();
    let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
    let pixels = Pixels::new(WIDTH, HEIGHT, surface_texture)?;
    let framework = Framework::new(window_size.width, window_size.height, scale_factor, &pixels);
    (pixels, framework)
  };

  let args: Args = Docopt::new(USAGE)
    .and_then(|d| d.deserialize())
    .unwrap_or_else(|e| e.exit());

  let mut breakpoints_enabled = true;

  // I could probably abstract some of this...
  let (sample_tx, sample_rx) = mpsc::channel();
  let audio_device = AudioDevice::init(sample_rx);
  audio_device.stream.pause().unwrap();

  let mut nes = match Nes::new(
    audio_device.sample_rate as f32,
    &args.arg_rom,
    "nessers-main/src/test_fixtures/ntscpalette.pal",
  ) {
    Ok(n) => n,
    Err(msg) => panic!("{}", msg),
  };

  nes.breakpoints = args
    .arg_breakpoints
    .iter()
    .map(|s| u16::from_str_radix(s, 16).unwrap())
    .collect();

  nes.reset();
  nes.step();

  let min_audio_buffer_size = audio_device.min_buffer_size;
  let max_audio_buffer_size = audio_device.max_buffer_size;

  let mut audio_buffer: Vec<f32> = vec![];
  let mut nes_debugger = NesDebugger::new(WIDTH, HEIGHT);
  let mut egui_has_focus = false;
  let mut last_frame = Instant::now();
  // Handle input and drive UI & screen rendering:
  event_loop.run(move |event, _, control_flow| {
    if input.update(&event) {
      if !egui_has_focus {
        // Close events
        if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
          *control_flow = ControlFlow::Exit;
          return;
        }

        // Player 1 controls
        nes.peripherals.controllers[0].a = input.key_held(VirtualKeyCode::X);
        nes.peripherals.controllers[0].b = input.key_held(VirtualKeyCode::Z);
        nes.peripherals.controllers[0].select = input.key_held(VirtualKeyCode::RShift);
        nes.peripherals.controllers[0].start = input.key_held(VirtualKeyCode::Return);
        nes.peripherals.controllers[0].up = input.key_held(VirtualKeyCode::Up);
        nes.peripherals.controllers[0].down = input.key_held(VirtualKeyCode::Down);
        nes.peripherals.controllers[0].left = input.key_held(VirtualKeyCode::Left);
        nes.peripherals.controllers[0].right = input.key_held(VirtualKeyCode::Right);

        if input.key_pressed(VirtualKeyCode::R) {
          nes.reset();
        }

        if input.key_pressed(VirtualKeyCode::Space) {
          nes_debugger.playing = !nes_debugger.playing;
          if nes_debugger.playing {
            // Ensure we step past any breakpoints we may have been hanging on:
            nes.step();
            audio_device.stream.play().unwrap();
          } else {
            audio_device.stream.pause().unwrap();
          }
        }

        if input.key_pressed_os(VirtualKeyCode::F) {
          nes_debugger.playing = false;
          audio_device.stream.pause().unwrap();
          nes.frame();
        }

        if input.key_pressed_os(VirtualKeyCode::Period) {
          nes_debugger.playing = false;
          audio_device.stream.pause().unwrap();
          nes.clock();
        }

        if input.key_pressed_os(VirtualKeyCode::Slash) {
          nes_debugger.playing = false;
          audio_device.stream.pause().unwrap();
          nes.step();
        }

        if input.key_pressed(VirtualKeyCode::B) {
          breakpoints_enabled = !breakpoints_enabled;
          println!(
            "Breakpoints {}",
            if breakpoints_enabled {
              "enabled"
            } else {
              "disabled"
            }
          );
        }
      }

      // Update the scale factor
      if let Some(factor) = input.scale_factor() {
        scale_factor = factor;
        framework.scale_factor(factor);
      }

      // Resize the window
      if let Some(size) = input.window_resized() {
        if size.width > 0 && size.height > 0 {
          // Resize the surface texture
          pixels.resize_surface(size.width, size.height);
          framework.resize(size.width, size.height);

          // Resize the world
          let LogicalSize { width, height } = size.to_logical(scale_factor);
          nes_debugger.resize(width, height);
          pixels.resize_buffer(width, height);
        }
      }

      window.request_redraw();
    }

    match event {
      Event::WindowEvent { event, .. } => {
        // Update egui inputs
        framework.handle_event(&event);
      }
      // Draw the current frame
      Event::RedrawRequested(_) => {
        // Only render if we're playing and enough time has passed to run at
        // ~60hz; prevents from running too fast when on a display with > 60hz
        if nes_debugger.playing && last_frame.elapsed() > Duration::from_millis(16) {
          last_frame = Instant::now();
          // Run our clock until a frame is ready, gathering samples as we go...
          loop {
            // Prevent buffer overrun; this could result in a dropped frame:
            if audio_buffer.len() > max_audio_buffer_size {
              break;
            }

            // Break on breakpoints:
            if nes.clock() && breakpoints_enabled {
              nes_debugger.playing = false;
              audio_device.stream.pause().unwrap();
              break;
            }

            if nes.apu.sample_ready {
              audio_buffer.push(nes.apu.sample());
            }

            if nes.ppu.frame_complete && audio_buffer.len() > (min_audio_buffer_size * 30) {
              // Draw the world
              nes_debugger.draw(pixels.get_frame(), &nes);
              break;
            }
          }
        }

        let mut last_sample_idx = 0;
        // Send samples until there's nothing to receive:
        for i in 0..std::cmp::min(max_audio_buffer_size, audio_buffer.len()) {
          last_sample_idx = i;
          match sample_tx.send(audio_buffer[i]) {
            Ok(_) => { /* keep sending */ }
            Err(_) => {
              println!("Nothing receiving... buffer overrun?");
              break;
            }
          }
        }
        audio_buffer.drain(0..last_sample_idx);

        // Prepare Dear ImGui
        framework.prepare(&window, &mut nes, &mut egui_has_focus);

        // Render everything together
        let render_result = pixels.render_with(|encoder, render_target, context| {
          // Render the world texture
          context.scaling_renderer.render(encoder, render_target);

          framework.render(encoder, render_target, context)?;

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
      _ => (),
    }
  });
}

/// Tying it all together.
struct NesDebugger {
  width: i16,
  height: i16,
  playing: bool,
}

impl NesDebugger {
  /// Create a new `World` instance that can draw a moving box.
  pub fn new(width: u32, height: u32) -> Self {
    Self {
      width: width as i16,
      height: height as i16,
      playing: false,
    }
  }

  /// Resize the world
  pub fn resize(&mut self, width: u32, height: u32) {
    self.width = width as i16;
    self.height = height as i16;
  }

  /// Draw the `World` state to the frame buffer.
  ///
  /// Assumes the default texture format: `wgpu::TextureFormat::Rgba8UnormSrgb`
  pub fn draw(&mut self, frame: &mut [u8], nes: &Nes) {
    // For now, just always redraw:
    for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
      let x = (i % self.width as usize) / 2;
      let y = (i / self.width as usize) / 2;
      if x < SCREEN_W && y > 8 && y < (SCREEN_H + 8) {
        let ppu_screen_idx = (y - 8) * SCREEN_W + x;
        pixel.copy_from_slice(&nes.ppu.screen[ppu_screen_idx]);
      } else {
        pixel.copy_from_slice(&[0x00, 0x00, 0x00, 0xFF]);
      }
    }
  }
}
