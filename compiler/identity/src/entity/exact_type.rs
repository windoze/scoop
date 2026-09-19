use scoop_wire::{Encoder, HashError, WireEncode, domain_separated_cbor_hash_stream_length};

use super::{CallingConvention, Effect, NonEmptyVec};
use crate::ids::derive_persistent_id;
use crate::{
    ConeCoordinate, ConeIdentity, PersistentExactTypeId, PersistentGenericTypeId, PersistentTypeId,
};

mod decode;

pub use decode::{DecodedExactTypeKey, ExactTypeResolutionError};

const EXACT_TYPE_HASH_DOMAIN: &str = "scoop-exact-type-v1";

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExactTypeKey {
    Nominal(PersistentTypeId),
    NominalApplication {
        origin: PersistentGenericTypeId,
        arguments: NonEmptyVec<PersistentExactTypeId>,
    },
    Tuple(NonEmptyVec<PersistentExactTypeId>),
    Function {
        effect: Effect,
        parameters: Vec<PersistentExactTypeId>,
        result: PersistentExactTypeId,
    },
    RawPointer(PersistentExactTypeId),
    NativeFunctionPointer {
        calling_convention: CallingConvention,
        parameters: Vec<PersistentExactTypeId>,
        result: PersistentExactTypeId,
    },
}

impl ExactTypeKey {
    pub fn exact_type_dependencies(&self) -> Vec<PersistentExactTypeId> {
        match self {
            Self::Nominal(_) => Vec::new(),
            Self::NominalApplication { arguments, .. } | Self::Tuple(arguments) => {
                arguments.as_slice().to_vec()
            }
            Self::Function {
                parameters, result, ..
            }
            | Self::NativeFunctionPointer {
                parameters, result, ..
            } => {
                let mut dependencies = parameters.clone();
                dependencies.push(*result);
                dependencies
            }
            Self::RawPointer(pointee) => vec![*pointee],
        }
    }
}

impl WireEncode for ExactTypeKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Nominal(id) => encode_single_payload(encoder, 1, id),
            Self::NominalApplication { origin, arguments } => {
                encoder.map(3)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                origin.encode(encoder)?;
                encoder.field(2)?;
                encode_ids(encoder, arguments.as_slice())
            }
            Self::Tuple(elements) => {
                encoder.map(2)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                encode_ids(encoder, elements.as_slice())
            }
            Self::Function {
                effect,
                parameters,
                result,
            } => {
                encoder.map(4)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                effect.encode(encoder)?;
                encoder.field(2)?;
                encode_ids(encoder, parameters)?;
                encoder.field(3)?;
                result.encode(encoder)
            }
            Self::RawPointer(pointee) => encode_single_payload(encoder, 5, pointee),
            Self::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => {
                encoder.map(4)?;
                encode_tag(encoder, 6)?;
                encoder.field(1)?;
                calling_convention.encode(encoder)?;
                encoder.field(2)?;
                encode_ids(encoder, parameters)?;
                encoder.field(3)?;
                result.encode(encoder)
            }
        }
    }
}

impl PersistentExactTypeId {
    pub fn from_key(key: &ExactTypeKey) -> Result<Self, HashError> {
        derive_persistent_id(EXACT_TYPE_HASH_DOMAIN, key)
    }

    pub fn hash_stream_length(key: &ExactTypeKey) -> Result<u64, HashError> {
        domain_separated_cbor_hash_stream_length(EXACT_TYPE_HASH_DOMAIN, key)
    }
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_single_payload(
    encoder: &mut Encoder,
    tag: u64,
    id: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    id.encode(encoder)
}

fn encode_ids(
    encoder: &mut Encoder,
    ids: &[PersistentExactTypeId],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(ids.len() as u64)?;
    for id in ids {
        id.encode(encoder)?;
    }
    Ok(())
}

mod diagnostic;
pub use diagnostic::{
    CanonicalExactTypeDiagnosticName, ExactTypeDiagnosticCatalog, ExactTypeDiagnosticCatalogError,
    ExactTypeDiagnosticError, ExactTypeDiagnosticGraph,
};

#[cfg(test)]
mod tests;
