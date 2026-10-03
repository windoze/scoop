use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, DecodedCallableTemplateOrigin, PersistentIdResolver,
    PersistentKeyResolver, PersistentSourceContextId, SourceContextKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::parameters::IndexedCallableSourceParameterV1;
use super::{
    CallableSourceParameterListResolutionError, CanonicalCallableSourceParametersV1,
    DecodedCanonicalCallableSourceParametersV1, ExportDefaultTemplateIndexResolver,
    ExportDefaultTemplateKeyResolver,
};
use crate::{
    CallableDeclarationIdResolver, ExportDefaultTemplateKeyV1, SignatureTypeReferenceResolver,
};

mod semantics;

pub use semantics::{
    CallableSourceInterfaceSemanticAuthority, CallableSourceInterfaceSemanticValidationError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSourceInterfaceV1 {
    owner: CallableTemplateOrigin,
    parameters: CanonicalCallableSourceParametersV1,
}

impl CallableSourceInterfaceV1 {
    pub fn try_new(
        owner: CallableTemplateOrigin,
        parameters: CanonicalCallableSourceParametersV1,
    ) -> Result<Self, CallableSourceInterfaceBuildError> {
        if matches!(owner, CallableTemplateOrigin::Accessor(_)) {
            return Err(CallableSourceInterfaceBuildError::PropertyAccessorOwner);
        }
        for (position, parameter) in (0_u32..).zip(parameters.parameters()) {
            let Some(actual) = parameter.calling().template() else {
                continue;
            };
            let expected = ExportDefaultTemplateKeyV1::new(owner, position);
            if actual != expected {
                return Err(CallableSourceInterfaceBuildError::TemplateKey {
                    position,
                    expected,
                    actual,
                });
            }
        }
        Ok(Self { owner, parameters })
    }

    pub const fn owner(&self) -> CallableTemplateOrigin {
        self.owner
    }

    pub const fn parameters(&self) -> &CanonicalCallableSourceParametersV1 {
        &self.parameters
    }

    /// Builds a wire-only projection whose template keys are replaced by
    /// canonical table indices. Raw indices never enter this semantic record.
    pub fn index_templates<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedCallableSourceInterfaceV1<'_>, CallableSourceInterfaceIndexError<I::Error>>
    where
        I: ExportDefaultTemplateIndexResolver,
    {
        let mut parameters = Vec::with_capacity(self.parameters.parameters().len());
        for (index, parameter) in self.parameters.parameters().iter().enumerate() {
            parameters.push(
                parameter.index_template(resolver).map_err(|error| {
                    CallableSourceInterfaceIndexError::Parameter { index, error }
                })?,
            );
        }
        Ok(IndexedCallableSourceInterfaceV1 {
            owner: self.owner,
            parameters,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCallableSourceInterfaceV1 {
    owner: DecodedCallableTemplateOrigin,
    parameters: DecodedCanonicalCallableSourceParametersV1,
}

impl DecodedCallableSourceInterfaceV1 {
    pub fn resolve<R, T, E>(
        self,
        resolver: &mut R,
        templates: &mut T,
    ) -> Result<CallableSourceInterfaceV1, CallableSourceInterfaceResolutionError<E, T::Error>>
    where
        R: CallableDeclarationIdResolver<E>
            + SignatureTypeReferenceResolver<E>
            + PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>,
        T: ExportDefaultTemplateKeyResolver,
    {
        let owner = self
            .owner
            .resolve(resolver)
            .map_err(CallableSourceInterfaceResolutionError::Owner)?;
        let parameters = self
            .parameters
            .resolve(resolver, templates)
            .map_err(CallableSourceInterfaceResolutionError::Parameters)?;
        CallableSourceInterfaceV1::try_new(owner, parameters)
            .map_err(CallableSourceInterfaceResolutionError::Record)
    }
}

impl WireEncode for DecodedCallableSourceInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.parameters.encode(encoder)
    }
}

impl WireDecode for DecodedCallableSourceInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            owner: decoder.field(1, DecodedCallableTemplateOrigin::decode)?,
            parameters: decoder.field(2, DecodedCanonicalCallableSourceParametersV1::decode)?,
        })
    }
}

pub struct IndexedCallableSourceInterfaceV1<'a> {
    owner: CallableTemplateOrigin,
    parameters: Vec<IndexedCallableSourceParameterV1<'a>>,
}

impl WireEncode for IndexedCallableSourceInterfaceV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        encoder.array(self.parameters.len() as u64)?;
        for parameter in &self.parameters {
            parameter.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableSourceInterfaceBuildError {
    PropertyAccessorOwner,
    TemplateKey {
        position: u32,
        expected: ExportDefaultTemplateKeyV1,
        actual: ExportDefaultTemplateKeyV1,
    },
}

impl fmt::Display for CallableSourceInterfaceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PropertyAccessorOwner => {
                formatter.write_str("property accessors do not have source call interfaces")
            }
            Self::TemplateKey {
                position,
                expected,
                actual,
            } => write!(
                formatter,
                "source parameter {position} references default template {actual:?}, expected {expected:?}"
            ),
        }
    }
}

impl std::error::Error for CallableSourceInterfaceBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum CallableSourceInterfaceResolutionError<E, T> {
    Owner(E),
    Parameters(CallableSourceParameterListResolutionError<E, T>),
    Record(CallableSourceInterfaceBuildError),
}

impl<E: fmt::Display, T: fmt::Display> fmt::Display
    for CallableSourceInterfaceResolutionError<E, T>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Owner(error) => write!(formatter, "invalid source interface owner: {error}"),
            Self::Parameters(error) => write!(formatter, "invalid source parameters: {error}"),
            Self::Record(error) => write!(formatter, "invalid source interface: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static, T: std::error::Error + 'static> std::error::Error
    for CallableSourceInterfaceResolutionError<E, T>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum CallableSourceInterfaceIndexError<E> {
    Parameter { index: usize, error: E },
}

impl<E: fmt::Display> fmt::Display for CallableSourceInterfaceIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parameter { index, error } => {
                write!(formatter, "cannot index source parameter {index}: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CallableSourceInterfaceIndexError<E> {}

#[cfg(test)]
mod tests;
