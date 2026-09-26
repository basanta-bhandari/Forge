use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsMode {
    Auto,
    Kitty,
    Sixel,
    Ascii,
}

impl GraphicsMode {
    pub fn detect(self) -> Self {
        if self != Self::Auto {
            return self;
        }
        let term = std::env::var("TERM")
            .unwrap_or_default()
            .to_ascii_lowercase();
        let program = std::env::var("TERM_PROGRAM")
            .unwrap_or_default()
            .to_ascii_lowercase();
        if std::env::var_os("KITTY_WINDOW_ID").is_some()
            || term.contains("kitty")
            || program.contains("wezterm")
        {
            Self::Kitty
        } else if term.contains("sixel")
            || term.contains("mlterm")
            || term.contains("foot")
            || program.contains("contour")
        {
            Self::Sixel
        } else {
            Self::Ascii
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "AUTO",
            Self::Kitty => "KITTY",
            Self::Sixel => "SIXEL",
            Self::Ascii => "ASCII",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "kitty" | "pixel" => Some(Self::Kitty),
            "sixel" => Some(Self::Sixel),
            "ascii" | "text" => Some(Self::Ascii),
            _ => None,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::Kitty,
            Self::Kitty => Self::Sixel,
            Self::Sixel => Self::Ascii,
            Self::Ascii => Self::Auto,
        }
    }
}

pub fn kitty_image(
    rgb: &[u8],
    width: u32,
    height: u32,
    columns: u16,
    rows: u16,
    image_id: u32,
) -> Vec<u8> {
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("in-memory PNG header");
        writer.write_image_data(rgb).expect("in-memory PNG data");
    }
    let encoded = base64(&png);
    let mut output = Vec::with_capacity(encoded.len() + encoded.len() / 4096 * 32 + 128);
    for (index, chunk) in encoded.as_bytes().chunks(4096).enumerate() {
        let more = usize::from((index + 1) * 4096 < encoded.len());
        if index == 0 {
            output.extend_from_slice(
                format!(
                    "\x1b_Ga=T,f=100,c={columns},r={rows},i={image_id},p=1,z=-1,q=2,C=1,m={more};"
                )
                .as_bytes(),
            );
        } else {
            output.extend_from_slice(format!("\x1b_Gm={more};").as_bytes());
        }
        output.extend_from_slice(chunk);
        output.extend_from_slice(b"\x1b\\");
    }
    output
}

pub fn kitty_delete(image_id: u32) -> Vec<u8> {
    format!("\x1b_Ga=d,d=I,i={image_id},q=2\x1b\\").into_bytes()
}

pub fn sixel_image(rgb: &[u8], width: u32, height: u32) -> Vec<u8> {
    let width = width as usize;
    let height = height as usize;
    let indexed: Vec<u8> = rgb
        .chunks_exact(3)
        .map(|pixel| quantize(pixel[0], pixel[1], pixel[2]))
        .collect();
    let mut output = String::new();
    write!(output, "\x1bPq\"1;1;{width};{height}").unwrap();
    for index in 0..16u8 {
        let (r, g, b) = palette(index);
        write!(
            output,
            "#{index};2;{};{};{}",
            u16::from(r) * 100,
            u16::from(g) * 100 / 3,
            u16::from(b) * 100
        )
        .unwrap();
    }
    for band_y in (0..height).step_by(6) {
        for color in 0..16u8 {
            write!(output, "#{color}").unwrap();
            let mut run_char = 0u8;
            let mut run_length = 0usize;
            for x in 0..width {
                let mut bits = 0u8;
                for bit in 0..6 {
                    let y = band_y + bit;
                    if y < height && indexed[y * width + x] == color {
                        bits |= 1 << bit;
                    }
                }
                let character = 63 + bits;
                if run_length > 0 && character != run_char {
                    push_sixel_run(&mut output, run_char, run_length);
                    run_length = 0;
                }
                run_char = character;
                run_length += 1;
            }
            push_sixel_run(&mut output, run_char, run_length);
            output.push('$');
        }
        output.push('-');
    }
    output.push_str("\x1b\\");
    output.into_bytes()
}

fn push_sixel_run(output: &mut String, character: u8, length: usize) {
    if length >= 4 {
        write!(output, "!{length}{}", character as char).unwrap();
    } else {
        for _ in 0..length {
            output.push(character as char);
        }
    }
}

fn quantize(r: u8, g: u8, b: u8) -> u8 {
    ((r >> 7) << 3) | ((g >> 6) << 1) | (b >> 7)
}

fn palette(index: u8) -> (u8, u8, u8) {
    ((index >> 3) & 1, (index >> 1) & 3, index & 1)
}

fn base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        output.push(TABLE[((value >> 18) & 63) as usize] as char);
        output.push(TABLE[((value >> 12) & 63) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kitty_payload_contains_geometry_and_data() {
        let payload = kitty_image(&[255, 0, 0], 1, 1, 1, 1, 42);
        let text = String::from_utf8(payload).unwrap();
        assert!(text.contains("f=100,c=1,r=1,i=42,p=1"));
        assert!(text.len() > 40);
    }

    #[test]
    fn sixel_payload_has_dcs_and_terminator() {
        let payload = sixel_image(&[0, 255, 0], 1, 1);
        assert!(payload.starts_with(b"\x1bPq"));
        assert!(payload.ends_with(b"\x1b\\"));
    }
}
