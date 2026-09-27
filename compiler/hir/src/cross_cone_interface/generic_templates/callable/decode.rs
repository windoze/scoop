use std::fmt;

use scoop_identity::{DecodedSignatureTypeKey, SourceOriginResolutionError};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{ExportGenericCallableBodyV1, GenericCallableBodyBuildError, encode_parameter_indices};
use crate::{
    BinderUseListValidationError, CallableSourceEffectsBuildError, DecodedCallableSourceEffectsV1,
    DecodedCanonicalBinderUseListV1, DecodedCanonicalTemplateLocalTableV1,
    DecodedDefaultCallableDeclarationV1, DecodedDefaultStatementV1,
    DecodedExportDefinitionSourceV1, DecodedGenericTemplatePredicatesV1,
    DefaultStatementReferenceResolver, DefaultStatementResolutionError, TemplateLocalLookupError,
    TemplateLocalSelectorResolver, TemplateLocalTableValidationError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedExportGenericCallableBodyV1 {
    owner: DecodedDefaultCallableDeclarationV1,
    locals: DecodedCanonicalTemplateLocalTableV1,
    parameters: Vec<u32>,
    statements: Vec<DecodedDefaultStatementV1>,
    result: DecodedSignatureTypeKey,
    effects: DecodedCallableSourceEffectsV1,
    type_parameters: DecodedCanonicalBinderUseListV1,
    predicates: DecodedGenericTemplatePredicatesV1,
    definition_origin: DecodedExportDefinitionSourceV1,
}

impl DecodedExportGenericCallableBodyV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExportGenericCallableBodyV1, GenericCallableBodyResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E>,
    {
        use GenericCallableBodyResolutionError as Error;
        let owner = self.owner.resolve(resolver).map_err(Error::Owner)?;
        let mut locals = self.locals.resolve(resolver).map_err(Error::Locals)?;
        let parameters = self
            .parameters
            .into_iter()
            .enumerate()
            .map(|(index, parameter)| {
                locals
                    .resolve_template_local_selector(parameter)
                    .map_err(|source| Error::Parameter { index, source })
            })
            .collect::<Result<_, _>>()?;
        let statements = self
            .statements
            .into_iter()
            .enumerate()
            .map(|(index, statement)| {
                statement
                    .resolve(resolver, &mut locals)
                    .map_err(|source| Error::Statement {
                        index,
                        source: Box::new(source),
                    })
            })
            .collect::<Result<_, _>>()?;
        let result = self.result.resolve(resolver).map_err(Error::Result)?;
        let effects = self.effects.validate().map_err(Error::Effects)?;
        let type_parameters = self
            .type_parameters
            .resolve(resolver)
            .map_err(Error::TypeParameters)?;
        let predicates = self
            .predicates
            .resolve(resolver)
            .map_err(Error::Predicates)?;
        let definition_origin = self
            .definition_origin
            .resolve(resolver)
            .map_err(Error::Origin)?;
        ExportGenericCallableBodyV1::try_new(
            owner,
            locals,
            parameters,
            statements,
            result,
            effects,
            type_parameters,
            predicates,
            definition_origin,
        )
        .map_err(Error::Record)
    }
}

impl WireEncode for DecodedExportGenericCallableBodyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.locals.encode(encoder)?;
        encode_parameter_indices(encoder, &self.parameters)?;
        encoder.field(4)?;
        encoder.array(self.statements.len() as u64)?;
        for statement in &self.statements {
            statement.encode(encoder)?;
        }
        encoder.field(5)?;
        self.result.encode(encoder)?;
        encoder.field(6)?;
        self.effects.encode(encoder)?;
        encoder.field(7)?;
        self.type_parameters.encode(encoder)?;
        encoder.field(8)?;
        self.predicates.encode(encoder)?;
        encoder.field(9)?;
        self.definition_origin.encode(encoder)
    }
}

impl WireDecode for DecodedExportGenericCallableBodyV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(9)?;
        Ok(Self {
            owner: decoder.field(1, DecodedDefaultCallableDeclarationV1::decode)?,
            locals: decoder.field(2, DecodedCanonicalTemplateLocalTableV1::decode)?,
            parameters: decoder.field(3, |decoder| {
                decoder.decode_array(|decoder, _| decoder.u32())
            })?,
            statements: decoder.field(4, |decoder| {
                decoder.decode_array(|decoder, _| DecodedDefaultStatementV1::decode(decoder))
            })?,
            result: decoder.field(5, DecodedSignatureTypeKey::decode)?,
            effects: decoder.field(6, DecodedCallableSourceEffectsV1::decode)?,
            type_parameters: decoder.field(7, DecodedCanonicalBinderUseListV1::decode)?,
            predicates: decoder.field(8, DecodedGenericTemplatePredicatesV1::decode)?,
            definition_origin: decoder.field(9, DecodedExportDefinitionSourceV1::decode)?,
        })
    }
}

#[derive(Debug)]
pub enum GenericCallableBodyResolutionError<E> {
    Owner(E),
    Locals(TemplateLocalTableValidationError<E>),
    Parameter {
        index: usize,
        source: TemplateLocalLookupError,
    },
    Statement {
        index: usize,
        source: Box<DefaultStatementResolutionError<E, TemplateLocalLookupError>>,
    },
    Result(E),
    Effects(CallableSourceEffectsBuildError),
    TypeParameters(BinderUseListValidationError<E>),
    Predicates(BinderUseListValidationError<E>),
    Origin(SourceOriginResolutionError<E>),
    Record(GenericCallableBodyBuildError),
}

impl<E: fmt::Display> fmt::Display for GenericCallableBodyResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Owner(source) => write!(formatter, "invalid generic body owner: {source}"),
            Self::Locals(source) => source.fmt(formatter),
            Self::Parameter { index, source } => {
                write!(formatter, "invalid generic parameter {index}: {source}")
            }
            Self::Statement { index, source } => {
                write!(formatter, "invalid generic statement {index}: {source}")
            }
            Self::Result(source) => write!(formatter, "invalid generic result: {source}"),
            Self::Effects(source) => source.fmt(formatter),
            Self::TypeParameters(source) | Self::Predicates(source) => source.fmt(formatter),
            Self::Origin(source) => source.fmt(formatter),
            Self::Record(source) => source.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for GenericCallableBodyResolutionError<E> {}

#[cfg(test)]
mod tests;
