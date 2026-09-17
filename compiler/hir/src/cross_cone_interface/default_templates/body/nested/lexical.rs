use std::fmt;

use scoop_identity::{
    DecodedPersistentId, DecodedSignatureTypeKey, PersistentGeneratedCallableId, SignatureTypeKey,
    StructuralDefinitionPath,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    DecodedDefaultCallableBodyTypeArgumentsV1, DecodedDefaultCaptureV1,
    DefaultCallableBodyTypeArgumentsResolutionError, DefaultCallableBodyTypeArgumentsV1,
    DefaultCaptureIndexError, DefaultCaptureResolutionError, DefaultCaptureV1,
    DefaultNestedCallableReferenceResolver, IndexedDefaultCaptureV1,
};
use crate::{TemplateLocalIndexResolver, TemplateLocalSelectorResolver};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct DefaultLexicalCallableV1 {
    body: PersistentGeneratedCallableId,
    definition_path: StructuralDefinitionPath,
    function_type: SignatureTypeKey,
    body_type_arguments: DefaultCallableBodyTypeArgumentsV1,
    captures: Vec<DefaultCaptureV1>,
    capture_count: u32,
    owner_type_parameter_count: u32,
}

impl DefaultLexicalCallableV1 {
    fn try_new(
        body: PersistentGeneratedCallableId,
        definition_path: StructuralDefinitionPath,
        function_type: SignatureTypeKey,
        body_type_arguments: DefaultCallableBodyTypeArgumentsV1,
        captures: Vec<DefaultCaptureV1>,
        owner_type_parameter_count: u32,
    ) -> Result<Self, DefaultLexicalCallableBuildError> {
        let capture_count = u32::try_from(captures.len())
            .map_err(|_| DefaultLexicalCallableBuildError::TooManyCaptures)?;
        Ok(Self {
            body,
            definition_path,
            function_type,
            body_type_arguments,
            captures,
            capture_count,
            owner_type_parameter_count,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultLambdaV1(DefaultLexicalCallableV1);

impl DefaultLambdaV1 {
    pub fn try_new(
        body: PersistentGeneratedCallableId,
        definition_path: StructuralDefinitionPath,
        function_type: SignatureTypeKey,
        body_type_arguments: DefaultCallableBodyTypeArgumentsV1,
        captures: Vec<DefaultCaptureV1>,
        owner_type_parameter_count: u32,
    ) -> Result<Self, DefaultLexicalCallableBuildError> {
        DefaultLexicalCallableV1::try_new(
            body,
            definition_path,
            function_type,
            body_type_arguments,
            captures,
            owner_type_parameter_count,
        )
        .map(Self)
    }

    pub const fn body(&self) -> PersistentGeneratedCallableId {
        self.0.body
    }

    pub const fn definition_path(&self) -> &StructuralDefinitionPath {
        &self.0.definition_path
    }

    pub const fn function_type(&self) -> &SignatureTypeKey {
        &self.0.function_type
    }

    pub const fn body_type_arguments(&self) -> &DefaultCallableBodyTypeArgumentsV1 {
        &self.0.body_type_arguments
    }

    pub fn captures(&self) -> &[DefaultCaptureV1] {
        &self.0.captures
    }

    pub const fn capture_count(&self) -> u32 {
        self.0.capture_count
    }

    pub const fn owner_type_parameter_count(&self) -> u32 {
        self.0.owner_type_parameter_count
    }

    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultLambdaV1<'_>, DefaultLexicalCallableIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        index_captures(&self.0, resolver).map(|captures| IndexedDefaultLambdaV1 {
            lambda: self,
            captures,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultAnonymousFunctionV1(DefaultLexicalCallableV1);

impl DefaultAnonymousFunctionV1 {
    pub fn try_new(
        body: PersistentGeneratedCallableId,
        definition_path: StructuralDefinitionPath,
        function_type: SignatureTypeKey,
        body_type_arguments: DefaultCallableBodyTypeArgumentsV1,
        captures: Vec<DefaultCaptureV1>,
        owner_type_parameter_count: u32,
    ) -> Result<Self, DefaultLexicalCallableBuildError> {
        DefaultLexicalCallableV1::try_new(
            body,
            definition_path,
            function_type,
            body_type_arguments,
            captures,
            owner_type_parameter_count,
        )
        .map(Self)
    }

    pub const fn body(&self) -> PersistentGeneratedCallableId {
        self.0.body
    }

    pub const fn definition_path(&self) -> &StructuralDefinitionPath {
        &self.0.definition_path
    }

    pub const fn function_type(&self) -> &SignatureTypeKey {
        &self.0.function_type
    }

    pub const fn body_type_arguments(&self) -> &DefaultCallableBodyTypeArgumentsV1 {
        &self.0.body_type_arguments
    }

    pub fn captures(&self) -> &[DefaultCaptureV1] {
        &self.0.captures
    }

    pub const fn capture_count(&self) -> u32 {
        self.0.capture_count
    }

    pub const fn owner_type_parameter_count(&self) -> u32 {
        self.0.owner_type_parameter_count
    }

    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultAnonymousFunctionV1<'_>, DefaultLexicalCallableIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        index_captures(&self.0, resolver).map(|captures| IndexedDefaultAnonymousFunctionV1 {
            function: self,
            captures,
        })
    }
}

fn index_captures<'a, I>(
    callable: &'a DefaultLexicalCallableV1,
    resolver: &mut I,
) -> Result<Vec<IndexedDefaultCaptureV1<'a>>, DefaultLexicalCallableIndexError<I::Error>>
where
    I: TemplateLocalIndexResolver,
{
    let mut captures = Vec::with_capacity(callable.capture_count as usize);
    for (index, capture) in callable.captures.iter().enumerate() {
        captures.push(
            capture
                .index_local(resolver)
                .map_err(|error| DefaultLexicalCallableIndexError::Capture { index, error })?,
        );
    }
    Ok(captures)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedDefaultLexicalCallableV1 {
    body: DecodedPersistentId<PersistentGeneratedCallableId>,
    definition_path: StructuralDefinitionPath,
    function_type: DecodedSignatureTypeKey,
    body_type_arguments: DecodedDefaultCallableBodyTypeArgumentsV1,
    captures: Vec<DecodedDefaultCaptureV1>,
    owner_type_parameter_count: u32,
}

impl DecodedDefaultLexicalCallableV1 {
    fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultLexicalCallableV1, DefaultLexicalCallableResolutionError<E, L::Error>>
    where
        R: DefaultNestedCallableReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        let body = resolver
            .resolve(self.body)
            .map_err(DefaultLexicalCallableResolutionError::Body)?;
        let function_type = self
            .function_type
            .resolve(resolver)
            .map_err(DefaultLexicalCallableResolutionError::FunctionType)?;
        let body_type_arguments = self
            .body_type_arguments
            .resolve(resolver)
            .map_err(DefaultLexicalCallableResolutionError::BodyTypeArguments)?;
        let capture_count = u32::try_from(self.captures.len()).map_err(|_| {
            DefaultLexicalCallableResolutionError::Record(
                DefaultLexicalCallableBuildError::TooManyCaptures,
            )
        })?;
        let mut captures = Vec::with_capacity(capture_count as usize);
        for (index, capture) in self.captures.into_iter().enumerate() {
            captures.push(capture.resolve(resolver, locals).map_err(|error| {
                DefaultLexicalCallableResolutionError::Capture { index, error }
            })?);
        }
        DefaultLexicalCallableV1::try_new(
            body,
            self.definition_path,
            function_type,
            body_type_arguments,
            captures,
            self.owner_type_parameter_count,
        )
        .map_err(DefaultLexicalCallableResolutionError::Record)
    }
}

impl WireEncode for DecodedDefaultLexicalCallableV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_lexical_callable(
            encoder,
            &self.body,
            &self.definition_path,
            &self.function_type,
            &self.body_type_arguments,
            &self.captures,
            self.owner_type_parameter_count,
        )
    }
}

impl WireDecode for DecodedDefaultLexicalCallableV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            body: decoder.field(1, DecodedPersistentId::decode)?,
            definition_path: decoder.field(2, StructuralDefinitionPath::decode)?,
            function_type: decoder.field(3, DecodedSignatureTypeKey::decode)?,
            body_type_arguments: decoder
                .field(4, DecodedDefaultCallableBodyTypeArgumentsV1::decode)?,
            captures: decoder.field(5, |decoder| {
                decoder.decode_array(|decoder, _| DecodedDefaultCaptureV1::decode(decoder))
            })?,
            owner_type_parameter_count: decoder.field(6, Decoder::u32)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultLambdaV1(DecodedDefaultLexicalCallableV1);

impl DecodedDefaultLambdaV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultLambdaV1, DefaultLexicalCallableResolutionError<E, L::Error>>
    where
        R: DefaultNestedCallableReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        self.0.resolve(resolver, locals).map(DefaultLambdaV1)
    }
}

impl WireEncode for DecodedDefaultLambdaV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultLambdaV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        DecodedDefaultLexicalCallableV1::decode(decoder).map(Self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultAnonymousFunctionV1(DecodedDefaultLexicalCallableV1);

impl DecodedDefaultAnonymousFunctionV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultAnonymousFunctionV1, DefaultLexicalCallableResolutionError<E, L::Error>>
    where
        R: DefaultNestedCallableReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        self.0
            .resolve(resolver, locals)
            .map(DefaultAnonymousFunctionV1)
    }
}

impl WireEncode for DecodedDefaultAnonymousFunctionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultAnonymousFunctionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        DecodedDefaultLexicalCallableV1::decode(decoder).map(Self)
    }
}

#[derive(Debug)]
pub struct IndexedDefaultLambdaV1<'a> {
    lambda: &'a DefaultLambdaV1,
    captures: Vec<IndexedDefaultCaptureV1<'a>>,
}

impl WireEncode for IndexedDefaultLambdaV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_indexed_lexical_callable(encoder, &self.lambda.0, &self.captures)
    }
}

#[derive(Debug)]
pub struct IndexedDefaultAnonymousFunctionV1<'a> {
    function: &'a DefaultAnonymousFunctionV1,
    captures: Vec<IndexedDefaultCaptureV1<'a>>,
}

impl WireEncode for IndexedDefaultAnonymousFunctionV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_indexed_lexical_callable(encoder, &self.function.0, &self.captures)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultLexicalCallableBuildError {
    TooManyCaptures,
}

impl fmt::Display for DefaultLexicalCallableBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyCaptures => {
                formatter.write_str("default lexical callable capture count exceeds u32")
            }
        }
    }
}

impl std::error::Error for DefaultLexicalCallableBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultLexicalCallableResolutionError<E, L> {
    Body(E),
    FunctionType(E),
    BodyTypeArguments(DefaultCallableBodyTypeArgumentsResolutionError<E>),
    Capture {
        index: usize,
        error: DefaultCaptureResolutionError<E, L>,
    },
    Record(DefaultLexicalCallableBuildError),
}

impl<E: fmt::Display, L: fmt::Display> fmt::Display
    for DefaultLexicalCallableResolutionError<E, L>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Body(error) => {
                write!(formatter, "invalid default lexical callable body: {error}")
            }
            Self::FunctionType(error) => {
                write!(formatter, "invalid default lexical callable type: {error}")
            }
            Self::BodyTypeArguments(error) => write!(
                formatter,
                "invalid default lexical callable body type arguments: {error}"
            ),
            Self::Capture { index, error } => {
                write!(
                    formatter,
                    "invalid default lexical callable capture {index}: {error}"
                )
            }
            Self::Record(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static, L: std::error::Error + 'static> std::error::Error
    for DefaultLexicalCallableResolutionError<E, L>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultLexicalCallableIndexError<E> {
    Capture {
        index: usize,
        error: DefaultCaptureIndexError<E>,
    },
}

impl<E: fmt::Display> fmt::Display for DefaultLexicalCallableIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capture { index, error } => write!(
                formatter,
                "cannot index default lexical callable capture {index}: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultLexicalCallableIndexError<E> {}

fn encode_indexed_lexical_callable(
    encoder: &mut Encoder,
    callable: &DefaultLexicalCallableV1,
    captures: &[IndexedDefaultCaptureV1<'_>],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encode_lexical_callable(
        encoder,
        &callable.body,
        &callable.definition_path,
        &callable.function_type,
        &callable.body_type_arguments,
        captures,
        callable.owner_type_parameter_count,
    )
}

fn encode_lexical_callable(
    encoder: &mut Encoder,
    body: &impl WireEncode,
    definition_path: &impl WireEncode,
    function_type: &impl WireEncode,
    body_type_arguments: &impl WireEncode,
    captures: &[impl WireEncode],
    owner_type_parameter_count: u32,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(6)?;
    encoder.field(1)?;
    body.encode(encoder)?;
    encoder.field(2)?;
    definition_path.encode(encoder)?;
    encoder.field(3)?;
    function_type.encode(encoder)?;
    encoder.field(4)?;
    body_type_arguments.encode(encoder)?;
    encoder.field(5)?;
    encode_sequence(encoder, captures)?;
    encoder.field(6)?;
    encoder.unsigned(u64::from(owner_type_parameter_count))
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
