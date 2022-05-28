extern crate cpal;

use std::sync::mpsc::Receiver;

use cpal::traits::{DeviceTrait, HostTrait};

pub struct AudioDevice {
  pub stream: cpal::Stream,
  pub min_buffer_size: usize,
  pub max_buffer_size: usize,
}

impl AudioDevice {
  pub fn init(rx: Receiver<f32>) -> Self {
    let host = cpal::default_host();
    let device = host.default_output_device().unwrap();
    println!("Output device: {}", device.name().unwrap());

    // Force 44.1kHz
    let config = device
      .supported_output_configs()
      .unwrap()
      .next()
      .unwrap()
      .with_sample_rate(cpal::SampleRate(44100));

    let buffer_size = config.buffer_size().clone();
    println!("Default output config: {:?}", config);

    let stream = match config.sample_format() {
      cpal::SampleFormat::F32 => run::<f32>(&device, &config.into(), rx),
      cpal::SampleFormat::I16 => run::<i16>(&device, &config.into(), rx),
      cpal::SampleFormat::U16 => run::<u16>(&device, &config.into(), rx),
    };

    AudioDevice {
      stream,
      min_buffer_size: min_buffer_size(&buffer_size),
      max_buffer_size: max_buffer_size(&buffer_size),
    }
  }
}

fn min_buffer_size(buffer_size: &cpal::SupportedBufferSize) -> usize {
  match *buffer_size {
    cpal::SupportedBufferSize::Range { min, .. } => min as usize,
    // Some sensible default:
    _ => 15,
  }
}

fn max_buffer_size(buffer_size: &cpal::SupportedBufferSize) -> usize {
  match *buffer_size {
    cpal::SupportedBufferSize::Range { max, .. } => max as usize,
    // Some sensible default:
    _ => 4096,
  }
}

pub fn run<T>(device: &cpal::Device, config: &cpal::StreamConfig, rx: Receiver<f32>) -> cpal::Stream
where
  T: cpal::Sample,
{
  let channels = config.channels as usize;

  let next_value = move || rx.recv().unwrap();

  // let next_value = move || match rx.try_recv() {
  //   Ok(v) => v,
  //   Err(_) => {
  //     // println!("Nothing sending...");
  //     0.0
  //   }
  // };

  device
    .build_output_stream(
      config,
      move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
        for frame in data.chunks_mut(channels) {
          let value: T = cpal::Sample::from::<f32>(&next_value());
          for sample in frame.iter_mut() {
            *sample = value;
          }
        }
      },
      |err| eprintln!("an error occurred on stream: {}", err),
    )
    .unwrap()
}
