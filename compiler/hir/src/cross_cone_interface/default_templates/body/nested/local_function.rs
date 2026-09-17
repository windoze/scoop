use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, DecodedCallableTemplateOrigin, SignatureTypeKey,
    StructuralDefinitionPath,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    DecodedDefaultCaptureV1, DefaultCaptureIndexError, DefaultCaptureResolutionError,
    DefaultCaptureV1, IndexedDefaultCaptureV1,
};
use crate::{
    DefaultCallableReferenceResolver, TemplateLocalIndexResolver, TemplateLocalReferenceResolver,
    TemplateLocalSelectorResolver,
};

/// Descriptor for a provider-owned local function body referenced by a
/// portable default template.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultLocalFunctionV1 {
    declaration: CallableTemplateOrigin,
    definition_path: StructuralDefinitionPath,
    function_type: SignatureTypeKey,
    captures: Vec<DefaultCaptureV1>,
    capture_count: u32,
    owner_type_parameter_count: u32,
}

impl DefaultLocalFunctionV1 {
    pub fn try_new(
        declaration: CallableTemplateOrigin,
        definition_path: StructuralDefinitionPath,
        function_type: SignatureTypeKey,
        captures: Vec<DefaultCaptureV1>,
        owner_type_parameter_count: u32,
    ) -> Result<Self, DefaultLocalFunctionBuildError> {
        require_local_function_declaration(declaration)?;
        let capture_count = u32::try_from(captures.len())
            .map_err(|_| DefaultLocalFunctionBuildError::TooManyCaptures)?;
        Ok(Self {
            declaration,
            definition_path,
            function_type,
            captures,
            capture_count,
            owner_type_parameter_count,
        })
    }

    pub const fn declaration(&self) -> CallableTemplateOrigin {
        self.declaration
    }

    pub const fn definition_path(&self) -> &StructuralDefinitionPath {
        &self.definition_path
    }

    pub const fn function_type(&self) -> &SignatureTypeKey {
        &self.function_type
    }

    pub fn captures(&self) -> &[DefaultCaptureV1] {
        &self.captures
    }

    pub const fn capture_count(&self) -> u32 {
        self.capture_count
    }

    pub const fn owner_type_parameter_count(&self) -> u32 {
        self.owner_type_parameter_count
    }

    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultLocalFunctionV1<'_>, DefaultLocalFunctionIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        let mut captures = Vec::with_capacity(self.capture_count as usize);
        for (index, capture) in self.captures.iter().enumerate() {
            captures.push(
                capture
                    .index_local(resolver)
                    .map_err(|error| DefaultLocalFunctionIndexError::Capture { index, error })?,
            );
        }
        Ok(IndexedDefaultLocalFunctionV1 {
            function: self,
            captures,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultLocalFunctionV1 {
    declaration: DecodedCallableTemplateOrigin,
    definition_path: StructuralDefinitionPath,
    function_type: scoop_identity::DecodedSignatureTypeKey,
    captures: Vec<DecodedDefaultCaptureV1>,
    owner_type_parameter_count: u32,
}

impl DecodedDefaultLocalFunctionV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultLocalFunctionV1, DefaultLocalFunctionResolutionError<E, L::Error>>
    where
        R: DefaultNestedCallableReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        let declaration = self
            .declaration
            .resolve(resolver)
            .map_err(DefaultLocalFunctionResolutionError::Declaration)?;
        let function_type = self
            .function_type
            .resolve(resolver)
            .map_err(DefaultLocalFunctionResolutionError::FunctionType)?;
        let capture_count = u32::try_from(self.captures.len()).map_err(|_| {
            DefaultLocalFunctionResolutionError::Record(
                DefaultLocalFunctionBuildError::TooManyCaptures,
            )
        })?;
        let mut captures = Vec::with_capacity(capture_count as usize);
        for (index, capture) in self.captures.into_iter().enumerate() {
            captures.push(
                capture.resolve(resolver, locals).map_err(|error| {
                    DefaultLocalFunctionResolutionError::Capture { index, error }
                })?,
            );
        }
        DefaultLocalFunctionV1::try_new(
            declaration,
            self.definition_path,
            function_type,
            captures,
            self.owner_type_parameter_count,
        )
        .map_err(DefaultLocalFunctionResolutionError::Record)
    }
}

impl WireEncode for DecodedDefaultLocalFunctionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.definition_path.encode(encoder)?;
        encoder.field(3)?;
        self.function_type.encode(encoder)?;
        encoder.field(4)?;
        encode_sequence(encoder, &self.captures)?;
        encoder.field(5)?;
        encoder.unsigned(u64::from(self.owner_type_parameter_count))
    }
}

impl WireDecode for DecodedDefaultLocalFunctionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            declaration: decoder.field(1, DecodedCallableTemplateOrigin::decode)?,
            definition_path: decoder.field(2, StructuralDefinitionPath::decode)?,
            function_type: decoder.field(3, scoop_identity::DecodedSignatureTypeKey::decode)?,
            captures: decoder.field(4, |decoder| {
                decoder.decode_array(|decoder, _| DecodedDefaultCaptureV1::decode(decoder))
            })?,
            owner_type_parameter_count: decoder.field(5, Decoder::u32)?,
        })
    }
}

#[derive(Debug)]
pub struct IndexedDefaultLocalFunctionV1<'a> {
    function: &'a DefaultLocalFunctionV1,
    captures: Vec<IndexedDefaultCaptureV1<'a>>,
}

impl WireEncode for IndexedDefaultLocalFunctionV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.function.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.function.definition_path.encode(encoder)?;
        encoder.field(3)?;
        self.function.function_type.encode(encoder)?;
        encoder.field(4)?;
        encode_sequence(encoder, &self.captures)?;
        encoder.field(5)?;
        encoder.unsigned(u64::from(self.function.owner_type_parameter_count))
    }
}

pub trait DefaultNestedCallableReferenceResolver<E>:
    DefaultCallableReferenceResolver<E> + TemplateLocalReferenceResolver<E>
{
}

impl<R, E> DefaultNestedCallableReferenceResolver<E> for R where
    R: DefaultCallableReferenceResolver<E> + TemplateLocalReferenceResolver<E>
{
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultLocalFunctionBuildError {
    UnsupportedDeclaration(CallableTemplateOrigin),
    TooManyCaptures,
}

impl fmt::Display for DefaultLocalFunctionBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedDeclaration(declaration) => write!(
                formatter,
                "default local function uses unsupported declaration {declaration:?}"
            ),
            Self::TooManyCaptures => {
                formatter.write_str("default local function capture count exceeds u32")
            }
        }
    }
}

impl std::error::Error for DefaultLocalFunctionBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultLocalFunctionResolutionError<E, L> {
    Declaration(E),
    FunctionType(E),
    Capture {
        index: usize,
        error: DefaultCaptureResolutionError<E, L>,
    },
    Record(DefaultLocalFunctionBuildError),
}

impl<E: fmt::Display, L: fmt::Display> fmt::Display for DefaultLocalFunctionResolutionError<E, L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declaration(error) => {
                write!(
                    formatter,
                    "invalid default local-function declaration: {error}"
                )
            }
            Self::FunctionType(error) => {
                write!(formatter, "invalid default local-function type: {error}")
            }
            Self::Capture { index, error } => {
                write!(
                    formatter,
                    "invalid default local-function capture {index}: {error}"
                )
            }
            Self::Record(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static, L: std::error::Error + 'static> std::error::Error
    for DefaultLocalFunctionResolutionError<E, L>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultLocalFunctionIndexError<E> {
    Capture {
        index: usize,
        error: DefaultCaptureIndexError<E>,
    },
}

impl<E: fmt::Display> fmt::Display for DefaultLocalFunctionIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capture { index, error } => {
                write!(
                    formatter,
                    "cannot index default local-function capture {index}: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultLocalFunctionIndexError<E> {}

fn require_local_function_declaration(
    declaration: CallableTemplateOrigin,
) -> Result<(), DefaultLocalFunctionBuildError> {
    match declaration {
        CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::GenericFunction(_) => Ok(()),
        declaration => Err(DefaultLocalFunctionBuildError::UnsupportedDeclaration(
            declaration,
        )),
    }
}

fn encode_sequence(
    encoder: &mut Encoder,
    values: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
