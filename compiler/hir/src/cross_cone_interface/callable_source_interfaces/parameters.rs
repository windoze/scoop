use std::collections::BTreeSet;

use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DecodedCanonicalIdentifier, DecodedSignatureTypeKey,
    PersistentIdResolver, PersistentKeyResolver, PersistentSourceContextId, SignatureTypeKey,
    SourceContextKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{
    DecodedExportDefinitionSourceV1, ExportDefaultTemplateKeyV1, ExportDefinitionSourceV1,
    SignatureTypeReferenceResolver,
};

mod errors;

pub use errors::{
    CallableParameterCallingResolutionError, CallableSourceParameterListBuildError,
    CallableSourceParameterListResolutionError, CallableSourceParameterResolutionError,
};

/// Resolves a wire-local default-template table index to its semantic key.
pub trait ExportDefaultTemplateKeyResolver {
    type Error;

    fn resolve_default_template_key(
        &mut self,
        index: u32,
    ) -> Result<ExportDefaultTemplateKeyV1, Self::Error>;
}

/// Maps a semantic default-template key to its canonical wire table index.
pub trait ExportDefaultTemplateIndexResolver {
    type Error;

    fn resolve_default_template_index(
        &mut self,
        key: ExportDefaultTemplateKeyV1,
    ) -> Result<u32, Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableParameterCallingV1 {
    Required,
    Default {
        template: ExportDefaultTemplateKeyV1,
    },
    VarargEmpty {
        element_type: SignatureTypeKey,
    },
    VarargDefault {
        element_type: SignatureTypeKey,
        template: ExportDefaultTemplateKeyV1,
    },
}

impl CallableParameterCallingV1 {
    pub const fn template(&self) -> Option<ExportDefaultTemplateKeyV1> {
        match self {
            Self::Default { template } | Self::VarargDefault { template, .. } => Some(*template),
            Self::Required | Self::VarargEmpty { .. } => None,
        }
    }

    pub const fn element_type(&self) -> Option<&SignatureTypeKey> {
        match self {
            Self::VarargEmpty { element_type } | Self::VarargDefault { element_type, .. } => {
                Some(element_type)
            }
            Self::Required | Self::Default { .. } => None,
        }
    }

    pub const fn is_vararg(&self) -> bool {
        Self::element_type(self).is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedCallableParameterCallingV1 {
    Required,
    Default {
        template_index: u32,
    },
    VarargEmpty {
        element_type: DecodedSignatureTypeKey,
    },
    VarargDefault {
        element_type: DecodedSignatureTypeKey,
        template_index: u32,
    },
}

impl DecodedCallableParameterCallingV1 {
    pub fn resolve<R, T, E>(
        self,
        resolver: &mut R,
        templates: &mut T,
    ) -> Result<CallableParameterCallingV1, CallableParameterCallingResolutionError<E, T::Error>>
    where
        R: SignatureTypeReferenceResolver<E>,
        T: ExportDefaultTemplateKeyResolver,
    {
        match self {
            Self::Required => Ok(CallableParameterCallingV1::Required),
            Self::Default { template_index } => templates
                .resolve_default_template_key(template_index)
                .map(|template| CallableParameterCallingV1::Default { template })
                .map_err(CallableParameterCallingResolutionError::Template),
            Self::VarargEmpty { element_type } => element_type
                .resolve(resolver)
                .map(|element_type| CallableParameterCallingV1::VarargEmpty { element_type })
                .map_err(CallableParameterCallingResolutionError::ElementType),
            Self::VarargDefault {
                element_type,
                template_index,
            } => {
                let element_type = element_type
                    .resolve(resolver)
                    .map_err(CallableParameterCallingResolutionError::ElementType)?;
                let template = templates
                    .resolve_default_template_key(template_index)
                    .map_err(CallableParameterCallingResolutionError::Template)?;
                Ok(CallableParameterCallingV1::VarargDefault {
                    element_type,
                    template,
                })
            }
        }
    }
}

impl WireEncode for DecodedCallableParameterCallingV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Required => encode_empty_sum(encoder, 1),
            Self::Default { template_index } => {
                encoder.map(2)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*template_index))
            }
            Self::VarargEmpty { element_type } => {
                encoder.map(2)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                element_type.encode(encoder)
            }
            Self::VarargDefault {
                element_type,
                template_index,
            } => {
                encoder.map(3)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                element_type.encode(encoder)?;
                encoder.field(2)?;
                encoder.unsigned(u64::from(*template_index))
            }
        }
    }
}

impl WireDecode for DecodedCallableParameterCallingV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Required)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, Decoder::u32)
                    .map(|template_index| Self::Default { template_index })
            }
            3 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedSignatureTypeKey::decode)
                    .map(|element_type| Self::VarargEmpty { element_type })
            }
            4 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::VarargDefault {
                    element_type: decoder.field(1, DecodedSignatureTypeKey::decode)?,
                    template_index: decoder.field(2, Decoder::u32)?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSourceParameterV1 {
    name: CanonicalIdentifier,
    value_type: SignatureTypeKey,
    calling: CallableParameterCallingV1,
    definition_origin: ExportDefinitionSourceV1,
}

impl CallableSourceParameterV1 {
    pub const fn new(
        name: CanonicalIdentifier,
        value_type: SignatureTypeKey,
        calling: CallableParameterCallingV1,
        definition_origin: ExportDefinitionSourceV1,
    ) -> Self {
        Self {
            name,
            value_type,
            calling,
            definition_origin,
        }
    }

    pub const fn name(&self) -> &CanonicalIdentifier {
        &self.name
    }

    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }

    pub const fn calling(&self) -> &CallableParameterCallingV1 {
        &self.calling
    }

    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }

    pub(super) fn index_template<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedCallableSourceParameterV1<'_>, I::Error>
    where
        I: ExportDefaultTemplateIndexResolver,
    {
        let calling = match &self.calling {
            CallableParameterCallingV1::Required => IndexedCallableParameterCallingV1::Required,
            CallableParameterCallingV1::Default { template } => {
                IndexedCallableParameterCallingV1::Default {
                    template_index: resolver.resolve_default_template_index(*template)?,
                }
            }
            CallableParameterCallingV1::VarargEmpty { element_type } => {
                IndexedCallableParameterCallingV1::VarargEmpty { element_type }
            }
            CallableParameterCallingV1::VarargDefault {
                element_type,
                template,
            } => IndexedCallableParameterCallingV1::VarargDefault {
                element_type,
                template_index: resolver.resolve_default_template_index(*template)?,
            },
        };
        Ok(IndexedCallableSourceParameterV1 {
            parameter: self,
            calling,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCallableSourceParameterV1 {
    name: DecodedCanonicalIdentifier,
    value_type: DecodedSignatureTypeKey,
    calling: DecodedCallableParameterCallingV1,
    definition_origin: DecodedExportDefinitionSourceV1,
}

impl DecodedCallableSourceParameterV1 {
    pub fn resolve<R, T, E>(
        self,
        resolver: &mut R,
        templates: &mut T,
    ) -> Result<CallableSourceParameterV1, CallableSourceParameterResolutionError<E, T::Error>>
    where
        R: SignatureTypeReferenceResolver<E>
            + PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>,
        T: ExportDefaultTemplateKeyResolver,
    {
        let name = self
            .name
            .validate()
            .map_err(CallableSourceParameterResolutionError::Name)?;
        let value_type = self
            .value_type
            .resolve(resolver)
            .map_err(CallableSourceParameterResolutionError::ValueType)?;
        let calling = self
            .calling
            .resolve(resolver, templates)
            .map_err(CallableSourceParameterResolutionError::Calling)?;
        let definition_origin = self
            .definition_origin
            .resolve(resolver)
            .map_err(CallableSourceParameterResolutionError::DefinitionOrigin)?;
        Ok(CallableSourceParameterV1::new(
            name,
            value_type,
            calling,
            definition_origin,
        ))
    }
}

impl WireEncode for DecodedCallableSourceParameterV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.name.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)?;
        encoder.field(3)?;
        self.calling.encode(encoder)?;
        encoder.field(4)?;
        self.definition_origin.encode(encoder)
    }
}

impl WireDecode for DecodedCallableSourceParameterV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            name: decoder.field(1, DecodedCanonicalIdentifier::decode)?,
            value_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            calling: decoder.field(3, DecodedCallableParameterCallingV1::decode)?,
            definition_origin: decoder.field(4, DecodedExportDefinitionSourceV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalCallableSourceParametersV1 {
    parameters: Vec<CallableSourceParameterV1>,
    len: u32,
}

impl CanonicalCallableSourceParametersV1 {
    pub fn try_new(
        parameters: Vec<CallableSourceParameterV1>,
    ) -> Result<Self, CallableSourceParameterListBuildError> {
        let len = u32::try_from(parameters.len())
            .map_err(|_| CallableSourceParameterListBuildError::TooMany)?;
        let mut names = BTreeSet::new();
        let mut first_vararg = None;
        for (position, parameter) in (0_u32..).zip(&parameters) {
            if !names.insert(parameter.name.clone()) {
                return Err(CallableSourceParameterListBuildError::DuplicateName {
                    position,
                    name: parameter.name.clone(),
                });
            }
            if parameter.calling.is_vararg()
                && let Some(first) = first_vararg.replace(position)
            {
                return Err(CallableSourceParameterListBuildError::MultipleVarargs {
                    first,
                    duplicate: position,
                });
            }
        }
        Ok(Self { parameters, len })
    }

    pub fn parameters(&self) -> &[CallableSourceParameterV1] {
        &self.parameters
    }

    pub const fn len_u32(&self) -> u32 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalCallableSourceParametersV1 {
    parameters: Vec<DecodedCallableSourceParameterV1>,
}

impl DecodedCanonicalCallableSourceParametersV1 {
    pub fn resolve<R, T, E>(
        self,
        resolver: &mut R,
        templates: &mut T,
    ) -> Result<
        CanonicalCallableSourceParametersV1,
        CallableSourceParameterListResolutionError<E, T::Error>,
    >
    where
        R: SignatureTypeReferenceResolver<E>
            + PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>,
        T: ExportDefaultTemplateKeyResolver,
    {
        let mut parameters = Vec::with_capacity(self.parameters.len());
        for (index, parameter) in self.parameters.into_iter().enumerate() {
            parameters.push(parameter.resolve(resolver, templates).map_err(|error| {
                CallableSourceParameterListResolutionError::Parameter { index, error }
            })?);
        }
        CanonicalCallableSourceParametersV1::try_new(parameters)
            .map_err(CallableSourceParameterListResolutionError::List)
    }
}

impl WireEncode for DecodedCanonicalCallableSourceParametersV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.parameters.len() as u64)?;
        for parameter in &self.parameters {
            parameter.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalCallableSourceParametersV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedCallableSourceParameterV1::decode(decoder))
            .map(|parameters| Self { parameters })
    }
}

pub(super) struct IndexedCallableSourceParameterV1<'a> {
    parameter: &'a CallableSourceParameterV1,
    calling: IndexedCallableParameterCallingV1<'a>,
}

enum IndexedCallableParameterCallingV1<'a> {
    Required,
    Default {
        template_index: u32,
    },
    VarargEmpty {
        element_type: &'a SignatureTypeKey,
    },
    VarargDefault {
        element_type: &'a SignatureTypeKey,
        template_index: u32,
    },
}

impl WireEncode for IndexedCallableSourceParameterV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.parameter.name.encode(encoder)?;
        encoder.field(2)?;
        self.parameter.value_type.encode(encoder)?;
        encoder.field(3)?;
        self.calling.encode(encoder)?;
        encoder.field(4)?;
        self.parameter.definition_origin.encode(encoder)
    }
}

impl WireEncode for IndexedCallableParameterCallingV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Required => encode_empty_sum(encoder, 1),
            Self::Default { template_index } => {
                encoder.map(2)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*template_index))
            }
            Self::VarargEmpty { element_type } => {
                encoder.map(2)?;
                encode_tag(encoder, 3)?;
                encoder.field(1)?;
                element_type.encode(encoder)
            }
            Self::VarargDefault {
                element_type,
                template_index,
            } => {
                encoder.map(3)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                element_type.encode(encoder)?;
                encoder.field(2)?;
                encoder.unsigned(u64::from(*template_index))
            }
        }
    }
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;
