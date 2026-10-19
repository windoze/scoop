use super::*;
use crate::{
    DecodedExportTemplateFragmentV1, ExportTemplateFragmentV1, TemplateFragmentResolutionError,
};
use scoop_identity::{DecodedSignatureTypeKey, SignatureTypeKey};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultRequirementBindingValueV1 {
    Forwarded(DefaultMemberRequirementRefV1),
    Implementation {
        body: ExportTemplateFragmentV1,
        /// Indices in the fragment's canonical local table, in receiver/argument order.
        inputs: Vec<u32>,
        type_arguments: Vec<SignatureTypeKey>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDefaultRequirementBindingValueV1 {
    Forwarded(DecodedDefaultMemberRequirementRefV1),
    Implementation {
        body: DecodedExportTemplateFragmentV1,
        inputs: Vec<u32>,
        type_arguments: Vec<DecodedSignatureTypeKey>,
    },
}

impl DecodedDefaultRequirementBindingValueV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultRequirementBindingValueV1, RequirementBindingResolutionError<E>>
    where
        R: crate::DefaultStatementReferenceResolver<E>,
    {
        use RequirementBindingResolutionError as Error;
        Ok(match self {
            Self::Forwarded(reference) => DefaultRequirementBindingValueV1::Forwarded(
                reference.resolve(resolver).map_err(Error::Reference)?,
            ),
            Self::Implementation {
                body,
                inputs,
                type_arguments,
            } => {
                let body = body
                    .resolve(resolver)
                    .map_err(|error| Error::Body(Box::new(error)))?;
                if body.results().len() != 1
                    || inputs
                        .iter()
                        .any(|&index| index as usize >= body.locals().records().len())
                {
                    return Err(Error::Shape);
                }
                let mut seen = std::collections::HashSet::new();
                if inputs.iter().any(|index| !seen.insert(*index)) {
                    return Err(Error::Shape);
                }
                let type_arguments = type_arguments
                    .into_iter()
                    .map(|ty| ty.resolve(resolver).map_err(Error::Reference))
                    .collect::<Result<_, _>>()?;
                DefaultRequirementBindingValueV1::Implementation {
                    body,
                    inputs,
                    type_arguments,
                }
            }
        })
    }
}

macro_rules! encode_binding {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                match self {
                    Self::Forwarded(reference) => {
                        encoder.map(2)?;
                        encoder.field(0)?;
                        encoder.unsigned(0)?;
                        encoder.field(1)?;
                        reference.encode(encoder)
                    }
                    Self::Implementation {
                        body,
                        inputs,
                        type_arguments,
                    } => {
                        encoder.map(4)?;
                        encoder.field(0)?;
                        encoder.unsigned(1)?;
                        encoder.field(1)?;
                        body.encode(encoder)?;
                        encoder.field(2)?;
                        encoder.array(inputs.len() as u64)?;
                        for &input in inputs {
                            encoder.unsigned(input.into())?;
                        }
                        encoder.field(3)?;
                        encoder.array(type_arguments.len() as u64)?;
                        for argument in type_arguments {
                            argument.encode(encoder)?;
                        }
                        Ok(())
                    }
                }
            }
        }
    };
}
encode_binding!(DefaultRequirementBindingValueV1);
encode_binding!(DecodedDefaultRequirementBindingValueV1);

impl WireDecode for DecodedDefaultRequirementBindingValueV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match (tag, fields) {
            (0, 2) => decoder
                .field(1, DecodedDefaultMemberRequirementRefV1::decode)
                .map(Self::Forwarded),
            (1, 4) => Ok(Self::Implementation {
                body: decoder.field(1, DecodedExportTemplateFragmentV1::decode)?,
                inputs: decoder.field(2, |d| d.decode_array(|d, _| d.u32()))?,
                type_arguments: decoder.field(3, |d| {
                    d.decode_array(|d, _| DecodedSignatureTypeKey::decode(d))
                })?,
            }),
            _ => Err(WireError::new(
                scoop_wire::WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                None,
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultRequirementBindingV1 {
    pub requirement: DefaultMemberRequirementRefV1,
    pub value: DefaultRequirementBindingValueV1,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultRequirementBindingV1 {
    requirement: DecodedDefaultMemberRequirementRefV1,
    value: DecodedDefaultRequirementBindingValueV1,
}
impl DecodedDefaultRequirementBindingV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultRequirementBindingV1, RequirementBindingResolutionError<E>>
    where
        R: crate::DefaultStatementReferenceResolver<E>,
    {
        Ok(DefaultRequirementBindingV1 {
            requirement: self
                .requirement
                .resolve(resolver)
                .map_err(RequirementBindingResolutionError::Reference)?,
            value: self.value.resolve(resolver)?,
        })
    }
}
macro_rules! encode_record {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                e.array(2)?;
                self.requirement.encode(e)?;
                self.value.encode(e)
            }
        }
    };
}
encode_record!(DefaultRequirementBindingV1);
encode_record!(DecodedDefaultRequirementBindingV1);
impl WireDecode for DecodedDefaultRequirementBindingV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        let actual = d.array()?;
        if actual != 2 {
            return Err(WireError::new(
                scoop_wire::WireErrorKind::InvalidLength {
                    expected: 2,
                    actual,
                },
                d.path().clone(),
                None,
            ));
        }
        Ok(Self {
            requirement: DecodedDefaultMemberRequirementRefV1::decode(d)?,
            value: DecodedDefaultRequirementBindingValueV1::decode(d)?,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum RequirementBindingResolutionError<E> {
    Reference(E),
    Body(Box<TemplateFragmentResolutionError<E>>),
    Shape,
}
impl<E: std::fmt::Display> std::fmt::Display for RequirementBindingResolutionError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reference(error) => write!(f, "invalid requirement reference: {error}"),
            Self::Body(error) => write!(f, "invalid requirement implementation: {error}"),
            Self::Shape => f.write_str("invalid requirement implementation input/result shape"),
        }
    }
}
