use encoding_rs::{Encoder, EncoderResult};

use super::kind::Kind;

pub enum Destination {
    Basic(Kind),
    Rs(Encoder),
}

impl Destination {
    pub fn new(kind: Kind) -> Self {
        match kind {
            Kind::Rs(encoding) => Self::Rs(encoding.new_encoder()),
            _ => Self::Basic(kind),
        }
    }

    pub fn write(&mut self, ch: char, output: &mut [u8; 32], replace: bool) -> Result<usize, i32> {
        match self {
            Self::Rs(encoder) => {
                let mut utf8 = [0; 4];
                let text = ch.encode_utf8(&mut utf8);
                let (result, read, written) =
                    encoder.encode_from_utf8_without_replacement(text, output, false);
                match result {
                    EncoderResult::InputEmpty if read == text.len() => Ok(written),
                    EncoderResult::Unmappable(_) if replace => {
                        let (result, read, size) = encoder.encode_from_utf8_without_replacement(
                            "?",
                            &mut output[written..],
                            false,
                        );
                        if result == EncoderResult::InputEmpty && read == 1 {
                            Ok(written + size)
                        } else {
                            Err(9)
                        }
                    }
                    EncoderResult::Unmappable(_) => Err(4),
                    _ => Err(9),
                }
            }
            Self::Basic(kind) => match kind {
                Kind::Ascii | Kind::Latin1 => {
                    let limit = if matches!(kind, Kind::Ascii) {
                        127
                    } else {
                        255
                    };
                    if ch as u32 > limit && !replace {
                        return Err(4);
                    }
                    output[0] = if ch as u32 > limit { b'?' } else { ch as u8 };
                    Ok(1)
                }
                Kind::Utf8 => Ok(ch.encode_utf8(output).len()),
                Kind::Utf16(order) => {
                    let mut units = [0; 2];
                    let units = ch.encode_utf16(&mut units);
                    for (index, unit) in units.iter().enumerate() {
                        let bytes = if *order == 1 {
                            unit.to_be_bytes()
                        } else {
                            unit.to_le_bytes()
                        };
                        output[index * 2..index * 2 + 2].copy_from_slice(&bytes);
                    }
                    Ok(units.len() * 2)
                }
                Kind::Utf32(order) => {
                    let bytes = if *order == 1 {
                        (ch as u32).to_be_bytes()
                    } else {
                        (ch as u32).to_le_bytes()
                    };
                    output[..4].copy_from_slice(&bytes);
                    Ok(4)
                }
                Kind::Rs(_) => Err(9),
            },
        }
    }

    pub fn finish(&mut self, output: &mut [u8; 32]) -> Result<usize, i32> {
        if let Self::Rs(encoder) = self {
            let (result, _, written) =
                encoder.encode_from_utf8_without_replacement("", output, true);
            if result == EncoderResult::InputEmpty {
                Ok(written)
            } else {
                Err(9)
            }
        } else {
            Ok(0)
        }
    }
}
