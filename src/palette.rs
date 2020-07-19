use std::fs;

/// 24-bit sRGB color
#[derive(Clone, Copy)]
pub struct Color {
  pub r: u8,
  pub g: u8,
  pub b: u8,
}

/// NES color palette
pub struct Palette {
  pub colors: [Color; 64],
}

impl Palette {
  pub fn from_file(filename: &str) -> Result<Palette, &'static str> {
    let contents = fs::read(filename).expect(&format!("Failure reading {}", filename));
    if contents.len() != 192 {
      return Err("File had size other than 192 (3 * 64) bytes");
    }

    let mut palette = Palette {
      colors: [Color { r: 0, g: 0, b: 0 }; 64],
    };
    let mut index = 0;
    while index < 192 {
      palette.colors[index / 3].r = contents[index + 0];
      palette.colors[index / 3].g = contents[index + 1];
      palette.colors[index / 3].b = contents[index + 2];
      index += 3;
    }

    Ok(palette)
  }
}
