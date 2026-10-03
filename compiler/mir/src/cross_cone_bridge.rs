//! Compile-facing MIR bridge for M23-5 ordinary dependency callables.

use scoop_wire::{Encoder, WireEncode};

mod errors;
mod model;
mod selection;
mod validation;
mod wire;

pub use errors::*;
pub use model::*;
pub use selection::*;
pub use wire::*;

fn encode_array<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
