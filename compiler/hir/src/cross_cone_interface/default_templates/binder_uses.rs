mod resolution_nodes;

use std::fmt;

use scoop_identity::{DecodedSignatureTypeKey, SignatureTypeKey};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::SignatureTypeReferenceResolver;

mod metered_substitution;
pub use metered_substitution::copy_default_signature_type_metered;
mod semantics;

pub use metered_substitution::MeteredDefaultTemplateTypeSubstitutionError;
pub use semantics::{BinderUseListSemanticValidationError, DefaultTemplateTypeSubstitutionError};

/// Declaration-order mapping from a default provider's binder slots to the
/// callable interface that publishes this template.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalBinderUseListV1 {
    arguments: Vec<SignatureTypeKey>,
    len: u32,
}

impl CanonicalBinderUseListV1 {
    pub fn try_new(arguments: Vec<SignatureTypeKey>) -> Result<Self, BinderUseListBuildError> {
        let len = u32::try_from(arguments.len()).map_err(|_| BinderUseListBuildError::TooMany)?;
        Ok(Self { arguments, len })
    }

    pub fn arguments(&self) -> &[SignatureTypeKey] {
        &self.arguments
    }

    pub fn len_u32(&self) -> u32 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.arguments.is_empty()
    }
}

impl WireEncode for CanonicalBinderUseListV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.arguments.len() as u64)?;
        for argument in &self.arguments {
            argument.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalBinderUseListV1 {
    arguments: Vec<DecodedSignatureTypeKey>,
}

impl DecodedCanonicalBinderUseListV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalBinderUseListV1, BinderUseListValidationError<E>>
    where
        R: SignatureTypeReferenceResolver<E>,
    {
        let len = u32::try_from(self.arguments.len())
            .map_err(|_| BinderUseListValidationError::TooMany)?;
        let mut arguments = Vec::with_capacity(self.arguments.len());
        for (index, argument) in self.arguments.into_iter().enumerate() {
            arguments.push(
                argument
                    .resolve(resolver)
                    .map_err(|error| BinderUseListValidationError::Reference { index, error })?,
            );
        }
        Ok(CanonicalBinderUseListV1 { arguments, len })
    }
}

impl WireEncode for DecodedCanonicalBinderUseListV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.arguments.len() as u64)?;
        for argument in &self.arguments {
            argument.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalBinderUseListV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedSignatureTypeKey::decode(decoder))
            .map(|arguments| Self { arguments })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinderUseListBuildError {
    TooMany,
}

impl fmt::Display for BinderUseListBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("default-template binder-use count exceeds u32")
    }
}

impl std::error::Error for BinderUseListBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum BinderUseListValidationError<E> {
    TooMany,
    Reference { index: usize, error: E },
}

impl<E: fmt::Display> fmt::Display for BinderUseListValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany => formatter.write_str("default-template binder-use count exceeds u32"),
            Self::Reference { index, error } => {
                write!(
                    formatter,
                    "invalid default-template binder use {index}: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for BinderUseListValidationError<E> {}

#[cfg(test)]
mod tests;
