use super::*;
use crate::{
    DecodedTemplateLocalDefinitionV1, DefaultNestedCallableReferenceResolver,
    TemplateLocalDefinitionV1,
};
use scoop_identity::{
    CallableTemplateOwner, DecodedCallableTemplateOwner, StructuralDefinitionPath,
};

/// A capture reads a value at creation while retaining its lexical binding.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultCaptureBindingV1 {
    Source,
    Definition {
        owner: CallableTemplateOwner,
        scope: StructuralDefinitionPath,
        selector: LocalValueSelector,
        definition: TemplateLocalDefinitionV1,
    },
}

impl WireEncode for DefaultCaptureBindingV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Source => {
                encoder.map(1)?;
                encode_tag(encoder, 1)
            }
            Self::Definition {
                owner,
                scope,
                selector,
                definition,
            } => {
                encoder.map(5)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                owner.encode(encoder)?;
                encoder.field(2)?;
                scope.encode(encoder)?;
                encoder.field(3)?;
                selector.encode(encoder)?;
                encoder.field(4)?;
                definition.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DecodedDefaultCaptureBindingV1 {
    Source,
    Definition {
        owner: DecodedCallableTemplateOwner,
        scope: StructuralDefinitionPath,
        selector: LocalValueSelector,
        definition: DecodedTemplateLocalDefinitionV1,
    },
}

impl DecodedDefaultCaptureBindingV1 {
    pub(super) fn resolve<R, E, L>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultCaptureBindingV1, DefaultCaptureResolutionError<E, L>>
    where
        R: DefaultNestedCallableReferenceResolver<E>,
    {
        match self {
            Self::Source => Ok(DefaultCaptureBindingV1::Source),
            Self::Definition {
                owner,
                scope,
                selector,
                definition,
            } => {
                let owner = owner
                    .resolve(resolver)
                    .map_err(DefaultCaptureResolutionError::BindingOwner)?;
                let definition = definition
                    .resolve(resolver)
                    .map_err(DefaultCaptureResolutionError::BindingOrigin)?;
                crate::cross_cone_interface::default_templates::locals::validate_definition_shape(
                    &selector,
                    &definition,
                )
                .map_err(DefaultCaptureResolutionError::BindingShape)?;
                Ok(DefaultCaptureBindingV1::Definition {
                    owner,
                    scope,
                    selector,
                    definition,
                })
            }
        }
    }
}

impl WireEncode for DecodedDefaultCaptureBindingV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Source => {
                encoder.map(1)?;
                encode_tag(encoder, 1)
            }
            Self::Definition {
                owner,
                scope,
                selector,
                definition,
            } => {
                encoder.map(5)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                owner.encode(encoder)?;
                encoder.field(2)?;
                scope.encode(encoder)?;
                encoder.field(3)?;
                selector.encode(encoder)?;
                encoder.field(4)?;
                definition.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedDefaultCaptureBindingV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Source)
            }
            2 => {
                expect_sum_length(decoder, fields, 5)?;
                Ok(Self::Definition {
                    owner: decoder.field(1, DecodedCallableTemplateOwner::decode)?,
                    scope: decoder.field(2, StructuralDefinitionPath::decode)?,
                    selector: decoder.field(3, LocalValueSelector::decode)?,
                    definition: decoder.field(4, DecodedTemplateLocalDefinitionV1::decode)?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
