extern crate cpal;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub struct AudioDevice {
  device: cpal::Device,
}

impl AudioDevice {
  pub fn init() -> Self {
    let host = cpal::default_host();
    let device = host.default_output_device().unwrap();

    AudioDevice { device }
  }

  pub fn play(&mut self) -> cpal::Stream {
    println!("Output device: {}", self.device.name().unwrap());

    let config = self.device.default_output_config().unwrap();
    println!("Default output config: {:?}", config);

    match config.sample_format() {
      cpal::SampleFormat::F32 => run::<f32>(&self.device, &config.into()),
      cpal::SampleFormat::I16 => run::<i16>(&self.device, &config.into()),
      cpal::SampleFormat::U16 => run::<u16>(&self.device, &config.into()),
    }
  }
}

pub fn run<T>(device: &cpal::Device, config: &cpal::StreamConfig) -> cpal::Stream
where
  T: cpal::Sample,
{
  let sample_rate = config.sample_rate.0 as f32;
  let channels = config.channels as usize;

  // Produce a sinusoid of maximum amplitude.
  let mut sample_clock = 0f32;
  let mut next_value = move || {
    sample_clock = (sample_clock + 1.0) % sample_rate;
    (sample_clock * 440.0 * 2.0 * std::f32::consts::PI / sample_rate).sin()
  };

  let stream = device
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
    .unwrap();
  stream.play().unwrap();
  stream
  // std::thread::sleep(std::time::Duration::from_millis(2000));
}
