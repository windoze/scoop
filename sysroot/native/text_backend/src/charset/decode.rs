use std::collections::VecDeque;

use encoding_rs::{Decoder, DecoderResult};

use super::{Failure, kind::Kind};

enum Codec {
    Rs(Decoder),
    Byte(u8),
    Utf32 {
        order: u8,
        bytes: [u8; 4],
        size: usize,
    },
}

pub struct Source {
    codec: Codec,
    pub offset: i64,
    unit_offset: i64,
    pub finished: bool,
}

impl Source {
    pub fn new(kind: Kind, offset: i64) -> Self {
        let codec = match kind {
            Kind::Ascii => Codec::Byte(127),
            Kind::Latin1 => Codec::Byte(255),
            Kind::Utf32(order) => Codec::Utf32 {
                order,
                bytes: [0; 4],
                size: 0,
            },
            Kind::Utf8 => Codec::Rs(encoding_rs::UTF_8.new_decoder_without_bom_handling()),
            Kind::Utf16(order) => Codec::Rs(
                if order == 1 {
                    encoding_rs::UTF_16BE
                } else {
                    encoding_rs::UTF_16LE
                }
                .new_decoder_without_bom_handling(),
            ),
            Kind::Rs(encoding) => Codec::Rs(encoding.new_decoder_without_bom_handling()),
        };
        Self {
            codec,
            offset,
            unit_offset: offset,
            finished: false,
        }
    }

    // Feed at most one byte so each emitted scalar retains its original source
    // unit offset, including multi-scalar Big5 mappings and stateful escapes.
    pub fn advance(
        &mut self,
        input: &[u8],
        end: bool,
        replace: bool,
        queue: &mut VecDeque<(char, i64)>,
    ) -> (usize, Option<Failure>) {
        if self.offset == i64::MAX && !input.is_empty() {
            return (0, Some(Failure::new(9, self.offset)));
        }
        let (read, malformed) = match &mut self.codec {
            Codec::Rs(decoder) => {
                let mut bytes = [0; 32];
                let (result, read, written) =
                    decoder.decode_to_utf8_without_replacement(input, &mut bytes, end);
                self.offset += read as i64;
                // The upstream decoder guarantees valid UTF-8 in its output.
                let Ok(text) = std::str::from_utf8(&bytes[..written]) else {
                    return (read, Some(Failure::new(9, self.offset)));
                };
                queue.extend(text.chars().map(|ch| (ch, self.unit_offset)));
                if written > 0 {
                    self.unit_offset = self.offset;
                }
                match result {
                    DecoderResult::Malformed(length, after) => {
                        let offset = self.offset - i64::from(length) - i64::from(after);
                        self.unit_offset = self.offset - i64::from(after);
                        (read, Some(Failure::new(if end { 5 } else { 3 }, offset)))
                    }
                    DecoderResult::InputEmpty => {
                        self.finished = end;
                        (read, None)
                    }
                    DecoderResult::OutputFull => (read, Some(Failure::new(9, self.offset))),
                }
            }
            Codec::Byte(max) => {
                if let Some(&byte) = input.first() {
                    let offset = self.offset;
                    self.offset += 1;
                    self.unit_offset = self.offset;
                    if byte <= *max {
                        queue.push_back((char::from(byte), offset));
                        (1, None)
                    } else {
                        (1, Some(Failure::new(3, offset)))
                    }
                } else {
                    self.finished = end;
                    (0, None)
                }
            }
            Codec::Utf32 { order, bytes, size } => {
                let read = if let Some(&byte) = input.first() {
                    bytes[*size] = byte;
                    *size += 1;
                    self.offset += 1;
                    1
                } else {
                    0
                };
                if *size == 4 {
                    *size = 0;
                    let scalar = if *order == 1 {
                        u32::from_be_bytes(*bytes)
                    } else {
                        u32::from_le_bytes(*bytes)
                    };
                    let origin = self.unit_offset;
                    self.unit_offset = self.offset;
                    if let Some(ch) = char::from_u32(scalar) {
                        queue.push_back((ch, origin));
                        (read, None)
                    } else {
                        (read, Some(Failure::new(3, origin)))
                    }
                } else if end && *size != 0 {
                    *size = 0;
                    (read, Some(Failure::new(5, self.unit_offset)))
                } else {
                    self.finished = end;
                    (read, None)
                }
            }
        };
        if let Some(error) = malformed {
            if replace && matches!(error.code, 3 | 5) {
                queue.push_back(('\u{fffd}', error.offset));
                return (read, None);
            }
            return (read, Some(error));
        }
        (read, None)
    }
}
