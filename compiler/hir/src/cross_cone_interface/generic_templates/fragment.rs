//! Typed evaluation fragments shared by constructor arguments and initialization.

use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::{
    CanonicalTemplateLocalTableV1, DecodedCanonicalTemplateLocalTableV1,
    DecodedDefaultExpressionV1, DecodedDefaultStatementV1, DefaultExpressionIndexError,
    DefaultExpressionResolutionError, DefaultExpressionV1, DefaultStatementIndexError,
    DefaultStatementReferenceResolver, DefaultStatementResolutionError, DefaultStatementV1,
    IndexedDefaultExpressionV1, IndexedDefaultStatementV1, TemplateLocalLookupError,
    TemplateLocalTableValidationError,
};

/// One original local scope, its evaluation order, and its actual result values.
/// A statement-only scope has no results; no synthetic value is required.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportTemplateFragmentV1 {
    locals: CanonicalTemplateLocalTableV1,
    statements: Vec<DefaultStatementV1>,
    results: Vec<DefaultExpressionV1>,
}

impl ExportTemplateFragmentV1 {
    pub fn new(
        locals: CanonicalTemplateLocalTableV1,
        statements: Vec<DefaultStatementV1>,
        results: Vec<DefaultExpressionV1>,
    ) -> Self {
        Self {
            locals,
            statements,
            results,
        }
    }

    pub const fn locals(&self) -> &CanonicalTemplateLocalTableV1 {
        &self.locals
    }

    pub fn statements(&self) -> &[DefaultStatementV1] {
        &self.statements
    }

    pub fn results(&self) -> &[DefaultExpressionV1] {
        &self.results
    }

    pub fn index_locals(
        &self,
    ) -> Result<IndexedExportTemplateFragmentV1<'_>, TemplateFragmentIndexError> {
        let mut locals = self.locals.clone();
        let statements = self
            .statements
            .iter()
            .enumerate()
            .map(|(index, statement)| {
                statement.index_locals(&mut locals).map_err(|source| {
                    TemplateFragmentIndexError::Statement {
                        index,
                        source: Box::new(source),
                    }
                })
            })
            .collect::<Result<_, _>>()?;
        let results = self
            .results
            .iter()
            .enumerate()
            .map(|(index, result)| {
                result.index_locals(&mut locals).map_err(|source| {
                    TemplateFragmentIndexError::Result {
                        index,
                        source: Box::new(source),
                    }
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(IndexedExportTemplateFragmentV1 {
            locals: &self.locals,
            statements,
            results,
        })
    }
}

pub struct IndexedExportTemplateFragmentV1<'a> {
    locals: &'a CanonicalTemplateLocalTableV1,
    statements: Vec<IndexedDefaultStatementV1<'a>>,
    results: Vec<IndexedDefaultExpressionV1<'a>>,
}

impl WireEncode for IndexedExportTemplateFragmentV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode(encoder, self.locals, &self.statements, &self.results)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedExportTemplateFragmentV1 {
    locals: DecodedCanonicalTemplateLocalTableV1,
    statements: Vec<DecodedDefaultStatementV1>,
    results: Vec<DecodedDefaultExpressionV1>,
}

impl DecodedExportTemplateFragmentV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExportTemplateFragmentV1, TemplateFragmentResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E>,
    {
        use TemplateFragmentResolutionError as Error;
        let mut locals = self.locals.resolve(resolver).map_err(Error::Locals)?;
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
        let results = self
            .results
            .into_iter()
            .enumerate()
            .map(|(index, result)| {
                result
                    .resolve(resolver, &mut locals)
                    .map_err(|source| Error::Result {
                        index,
                        source: Box::new(source),
                    })
            })
            .collect::<Result<_, _>>()?;
        Ok(ExportTemplateFragmentV1::new(locals, statements, results))
    }
}

impl WireEncode for DecodedExportTemplateFragmentV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode(encoder, &self.locals, &self.statements, &self.results)
    }
}

impl WireDecode for DecodedExportTemplateFragmentV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            locals: decoder.field(1, DecodedCanonicalTemplateLocalTableV1::decode)?,
            statements: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedDefaultStatementV1::decode(decoder))
            })?,
            results: decoder.field(3, |decoder| {
                decoder.decode_array(|decoder, _| DecodedDefaultExpressionV1::decode(decoder))
            })?,
        })
    }
}

fn encode<L: WireEncode, S: WireEncode, V: WireEncode>(
    encoder: &mut Encoder,
    locals: &L,
    statements: &[S],
    results: &[V],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encoder.field(1)?;
    locals.encode(encoder)?;
    encoder.field(2)?;
    encoder.array(statements.len() as u64)?;
    for statement in statements {
        statement.encode(encoder)?;
    }
    encoder.field(3)?;
    encoder.array(results.len() as u64)?;
    for result in results {
        result.encode(encoder)?;
    }
    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
pub enum TemplateFragmentIndexError {
    Statement {
        index: usize,
        source: Box<DefaultStatementIndexError<TemplateLocalLookupError>>,
    },
    Result {
        index: usize,
        source: Box<DefaultExpressionIndexError<TemplateLocalLookupError>>,
    },
}

impl fmt::Display for TemplateFragmentIndexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Statement { index, source } => {
                write!(formatter, "invalid template statement {index}: {source}")
            }
            Self::Result { index, source } => {
                write!(formatter, "invalid template result {index}: {source}")
            }
        }
    }
}
impl std::error::Error for TemplateFragmentIndexError {}

#[derive(Debug)]
pub enum TemplateFragmentResolutionError<E> {
    Locals(TemplateLocalTableValidationError<E>),
    Statement {
        index: usize,
        source: Box<DefaultStatementResolutionError<E, TemplateLocalLookupError>>,
    },
    Result {
        index: usize,
        source: Box<DefaultExpressionResolutionError<E, TemplateLocalLookupError>>,
    },
}

impl<E: fmt::Display> fmt::Display for TemplateFragmentResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Locals(source) => source.fmt(formatter),
            Self::Statement { index, source } => {
                write!(formatter, "invalid template statement {index}: {source}")
            }
            Self::Result { index, source } => {
                write!(formatter, "invalid template result {index}: {source}")
            }
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for TemplateFragmentResolutionError<E> {}
