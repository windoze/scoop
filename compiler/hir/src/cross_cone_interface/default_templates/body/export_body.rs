use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::{
    DecodedDefaultExpressionV1, DecodedDefaultStatementV1, DefaultExpressionIndexError,
    DefaultExpressionResolutionError, DefaultExpressionV1, DefaultStatementIndexError,
    DefaultStatementReferenceResolver, DefaultStatementResolutionError, DefaultStatementV1,
    IndexedDefaultExpressionV1, IndexedDefaultStatementV1, TemplateLocalIndexResolver,
    TemplateLocalSelectorResolver,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExportDefaultBodyV1 {
    statements: Vec<DefaultStatementV1>,
    value: DefaultExpressionV1,
}

impl ExportDefaultBodyV1 {
    pub fn try_new(
        statements: Vec<DefaultStatementV1>,
        value: DefaultExpressionV1,
    ) -> Result<Self, ExportDefaultBodyBuildError> {
        require_statement_count(statements.len())?;
        Ok(Self { statements, value })
    }

    pub fn statements(&self) -> &[DefaultStatementV1] {
        &self.statements
    }

    pub const fn value(&self) -> &DefaultExpressionV1 {
        &self.value
    }

    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedExportDefaultBodyV1<'_>, ExportDefaultBodyIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        let mut statements = Vec::with_capacity(self.statements.len());
        for (index, statement) in self.statements.iter().enumerate() {
            statements.push(
                statement
                    .index_locals(resolver)
                    .map_err(|error| ExportDefaultBodyIndexError::Statement { index, error })?,
            );
        }
        let value = self
            .value
            .index_locals(resolver)
            .map_err(ExportDefaultBodyIndexError::Value)?;
        Ok(IndexedExportDefaultBodyV1 { statements, value })
    }
}

#[derive(Debug)]
pub struct IndexedExportDefaultBodyV1<'a> {
    statements: Vec<IndexedDefaultStatementV1<'a>>,
    value: IndexedDefaultExpressionV1<'a>,
}

impl WireEncode for IndexedExportDefaultBodyV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_body(encoder, &self.statements, &self.value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedExportDefaultBodyV1 {
    statements: Vec<DecodedDefaultStatementV1>,
    value: DecodedDefaultExpressionV1,
}

impl DecodedExportDefaultBodyV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<ExportDefaultBodyV1, ExportDefaultBodyResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        require_statement_count(self.statements.len())
            .map_err(ExportDefaultBodyResolutionError::Record)?;
        let mut statements = Vec::with_capacity(self.statements.len());
        for (index, statement) in self.statements.into_iter().enumerate() {
            statements.push(
                statement.resolve(resolver, locals).map_err(|error| {
                    ExportDefaultBodyResolutionError::Statement { index, error }
                })?,
            );
        }
        let value = self
            .value
            .resolve(resolver, locals)
            .map_err(ExportDefaultBodyResolutionError::Value)?;
        ExportDefaultBodyV1::try_new(statements, value)
            .map_err(ExportDefaultBodyResolutionError::Record)
    }
}

impl WireEncode for DecodedExportDefaultBodyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_body(encoder, &self.statements, &self.value)
    }
}

impl WireDecode for DecodedExportDefaultBodyV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            statements: decoder.field(1, |decoder| {
                decoder.decode_array(|decoder, _| DecodedDefaultStatementV1::decode(decoder))
            })?,
            value: decoder.field(2, DecodedDefaultExpressionV1::decode)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportDefaultBodyBuildError {
    TooManyStatements,
}

impl fmt::Display for ExportDefaultBodyBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyStatements => {
                formatter.write_str("export default body statement count exceeds u32")
            }
        }
    }
}

impl std::error::Error for ExportDefaultBodyBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultBodyIndexError<E> {
    Statement {
        index: usize,
        error: DefaultStatementIndexError<E>,
    },
    Value(DefaultExpressionIndexError<E>),
}

impl<E: fmt::Display> fmt::Display for ExportDefaultBodyIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Statement { index, error } => {
                write!(
                    formatter,
                    "cannot index export default statement {index}: {error}"
                )
            }
            Self::Value(error) => write!(formatter, "cannot index export default value: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ExportDefaultBodyIndexError<E> {}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultBodyResolutionError<E, L> {
    Statement {
        index: usize,
        error: DefaultStatementResolutionError<E, L>,
    },
    Value(DefaultExpressionResolutionError<E, L>),
    Record(ExportDefaultBodyBuildError),
}

impl<E: fmt::Display, L: fmt::Display> fmt::Display for ExportDefaultBodyResolutionError<E, L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Statement { index, error } => {
                write!(
                    formatter,
                    "invalid export default statement {index}: {error}"
                )
            }
            Self::Value(error) => write!(formatter, "invalid export default value: {error}"),
            Self::Record(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static, L: std::error::Error + 'static> std::error::Error
    for ExportDefaultBodyResolutionError<E, L>
{
}

fn require_statement_count(length: usize) -> Result<(), ExportDefaultBodyBuildError> {
    u32::try_from(length)
        .map(|_| ())
        .map_err(|_| ExportDefaultBodyBuildError::TooManyStatements)
}

fn encode_body(
    encoder: &mut Encoder,
    statements: &[impl WireEncode],
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(1)?;
    encoder.array(statements.len() as u64)?;
    for statement in statements {
        statement.encode(encoder)?;
    }
    encoder.field(2)?;
    value.encode(encoder)
}

#[cfg(test)]
mod tests {
    use scoop_identity::LocalValueSelector;
    use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

    use super::*;
    use crate::{DefaultExpressionKindV1, DefaultStatementKindV1, OptionalDefaultExpressionV1};

    use super::super::expressions::test_support::{Fixture, LocalError};

    #[test]
    fn export_default_body_keeps_source_order_and_round_trips() {
        let fixture = Fixture::new();
        let body = ExportDefaultBodyV1::try_new(
            vec![
                statement(DefaultStatementKindV1::Break, &fixture),
                statement(DefaultStatementKindV1::Continue, &fixture),
                statement(
                    DefaultStatementKindV1::Return(OptionalDefaultExpressionV1::present(unit(
                        &fixture,
                    ))),
                    &fixture,
                ),
            ],
            unit(&fixture),
        )
        .unwrap();

        let bytes = encode(&body.index_locals(&mut fixture.locals()).unwrap()).unwrap();
        let decoded: DecodedExportDefaultBodyV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(
            decoded.resolve(&mut fixture.resolver(), &mut fixture.locals()),
            Ok(body)
        );
    }

    #[test]
    fn export_default_body_index_error_keeps_statement_position() {
        let fixture = Fixture::new();
        let missing_local = DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::Local(LocalValueSelector::Parameter {
                declaration_index: 1,
            }),
            fixture.value_type(),
            fixture.origin(),
        )
        .unwrap();
        let body = ExportDefaultBodyV1::try_new(
            vec![
                statement(DefaultStatementKindV1::Break, &fixture),
                statement(
                    DefaultStatementKindV1::Expr(Box::new(missing_local)),
                    &fixture,
                ),
            ],
            unit(&fixture),
        )
        .unwrap();

        assert_eq!(
            body.index_locals(&mut fixture.locals()).unwrap_err(),
            ExportDefaultBodyIndexError::Statement {
                index: 1,
                error: DefaultStatementIndexError::Expression {
                    variant_tag: 1,
                    field: 1,
                    error: DefaultExpressionIndexError::Local(LocalError),
                },
            }
        );
    }

    #[test]
    fn export_default_body_decoder_requires_exact_product() {
        let error = decode_canonical::<DecodedExportDefaultBodyV1>(
            &[0xa1, 0x01, 0x80],
            DecodeLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            error.kind(),
            &WireErrorKind::InvalidLength {
                expected: 2,
                actual: 1,
            }
        );
    }

    fn statement(kind: DefaultStatementKindV1, fixture: &Fixture) -> DefaultStatementV1 {
        DefaultStatementV1::try_new(kind, fixture.origin()).unwrap()
    }

    fn unit(fixture: &Fixture) -> DefaultExpressionV1 {
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::UnitLiteral,
            fixture.value_type(),
            fixture.origin(),
        )
        .unwrap()
    }
}
