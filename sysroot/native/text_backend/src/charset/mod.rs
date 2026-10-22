mod bom;
mod decode;
mod encode;
mod ffi;
mod kind;
mod names;

use std::collections::VecDeque;

use bom::Header;
use decode::Source;
use encode::Destination;
use kind::Kind;

#[repr(C)]
pub struct Options {
    input_bom: i32,
    source_order: i32,
    destination_order: i32,
    emit_bom: i32,
    replace_input: i32,
    replace_output: i32,
}

#[derive(Clone, Copy)]
struct Failure {
    code: i32,
    offset: i64,
}

impl Failure {
    fn new(code: i32, offset: i64) -> Self {
        Self { code, offset }
    }
}

#[repr(C)]
#[derive(Default)]
pub struct Step {
    consumed: i64,
    produced: i64,
    offset: i64,
    status: i32,
    code: i32,
    backend: i32,
}

struct Engine {
    kind: Kind,
    options: Options,
    header: Header,
    source: Option<Source>,
    destination: Destination,
    decoded: VecDeque<(char, i64)>,
    source_error: Option<Failure>,
    error: Option<Failure>,
    output: [u8; 32],
    output_read: usize,
    output_size: usize,
    finished: bool,
    ending: bool,
}

impl Engine {
    fn new(source: Kind, destination: Kind, options: Options) -> Result<Self, i32> {
        if !(0..=2).contains(&options.input_bom)
            || !(0..=2).contains(&options.source_order)
            || !(0..=2).contains(&options.destination_order)
            || ![
                options.emit_bom,
                options.replace_input,
                options.replace_output,
            ]
            .iter()
            .all(|n| (0..=1).contains(n))
            || (options.source_order != 0 && !source.ambiguous())
            || (options.destination_order != 0 && !destination.ambiguous())
            || (options.destination_order == 0 && destination.ambiguous())
            || (options.input_bom == 2 && source.bom_pair().0.is_empty())
            || (options.emit_bom == 1 && destination.bom_pair().0.is_empty())
        {
            return Err(8);
        }
        let destination = destination.ordered(options.destination_order as u8);
        let mut output = [0; 32];
        let bom = if options.emit_bom == 1 {
            destination.bom(0)
        } else {
            b""
        };
        output[..bom.len()].copy_from_slice(bom);
        Ok(Self {
            kind: source,
            options,
            header: Header::default(),
            source: None,
            destination: Destination::new(destination),
            decoded: VecDeque::with_capacity(8),
            source_error: None,
            error: None,
            output,
            output_read: 0,
            output_size: bom.len(),
            finished: false,
            ending: false,
        })
    }

    fn process(&mut self, input: &[u8], output: &mut [u8], end: bool) -> Step {
        let mut step = Step::default();
        let mut read = 0;
        let mut written = 0;
        if self.ending && !input.is_empty() {
            self.error = Some(Failure::new(
                8,
                self.source.as_ref().map_or(0, |s| s.offset),
            ));
        }
        loop {
            let count = (self.output_size - self.output_read).min(output.len() - written);
            output[written..written + count]
                .copy_from_slice(&self.output[self.output_read..self.output_read + count]);
            self.output_read += count;
            written += count;
            if self.output_read != self.output_size {
                step.status = 1;
                break;
            }
            self.output_read = 0;
            self.output_size = 0;
            if self.error.is_some() {
                break;
            }
            if self.finished {
                step.status = 2;
                break;
            }
            if written == output.len() {
                step.status = 1;
                break;
            }

            if let Some((ch, offset)) = self.decoded.pop_front() {
                match self
                    .destination
                    .write(ch, &mut self.output, self.options.replace_output == 1)
                {
                    Ok(size) => self.output_size = size,
                    Err(code) => self.error = Some(Failure::new(code, offset)),
                }
                continue;
            }
            if let Some(error) = self.source_error.take() {
                self.error = Some(error);
                continue;
            }
            if self.source.is_none() {
                match self
                    .header
                    .resolve(self.kind, &self.options, end && read == input.len())
                {
                    Ok(Some((kind, offset))) => self.source = Some(Source::new(kind, offset)),
                    Ok(None) => {
                        if read == input.len() {
                            break;
                        }
                        self.header.bytes[self.header.size] = input[read];
                        self.header.size += 1;
                        read += 1;
                        continue;
                    }
                    Err(code) => {
                        self.error = Some(Failure::new(code, 0));
                        continue;
                    }
                }
            }
            let source = self.source.as_mut().expect("resolved source header");
            if source.finished {
                match self.destination.finish(&mut self.output) {
                    Ok(size) => {
                        self.output_size = size;
                        self.finished = true;
                    }
                    Err(code) => self.error = Some(Failure::new(code, source.offset)),
                }
                continue;
            }
            let header_pending = self.header.read < self.header.size;
            let bytes = if header_pending {
                &self.header.bytes[self.header.read..self.header.read + 1]
            } else {
                &input[read..(read + 1).min(input.len())]
            };
            // Finalize only after all real bytes were passed with last=false.
            // Errors first exposed by the empty final call are incomplete tails.
            let last = bytes.is_empty() && (end || self.ending);
            let (used, error) = source.advance(
                bytes,
                last,
                self.options.replace_input == 1,
                &mut self.decoded,
            );
            let empty = bytes.is_empty();
            if header_pending {
                self.header.read += used;
            } else {
                read += used;
            }
            self.source_error = error;
            if empty && !last && self.decoded.is_empty() && error.is_none() {
                break;
            }
        }
        self.ending |= end && read == input.len();
        if let Some(error) = self.error {
            step.code = error.code;
            step.offset = error.offset;
        }
        step.consumed = read as i64;
        step.produced = written as i64;
        step
    }
}
