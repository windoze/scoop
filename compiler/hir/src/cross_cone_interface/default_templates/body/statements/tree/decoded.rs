use std::fmt;

use scoop_identity::{
    DecodedPersistentId, PersistentInitializationUnitId, SourceOriginResolutionError,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{DefaultStatementBuildError, DefaultStatementKindV1, DefaultStatementV1};
use crate::{
    DecodedDefaultAssignTargetV1, DecodedDefaultExpressionV1, DecodedDefaultLocalFunctionV1,
    DecodedDefaultPatternV1, DecodedExportDefinitionSourceV1, DecodedOptionalDefaultExpressionV1,
    DefaultAssignTargetResolutionError, DefaultExpressionResolutionError,
    DefaultLocalFunctionResolutionError, DefaultPatternResolutionError,
    OptionalDefaultExpressionV1, TemplateLocalSelectorResolver,
};

use super::super::{
    DecodedDefaultForIterationPlanV1, DecodedDefaultTryV1, DecodedDefaultWhenV1,
    DecodedOptionalDefaultStatementListV1, DefaultControlFlowResolutionError,
    DefaultForIterationPlanResolutionError, DefaultStatementReferenceResolver,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultStatementV1 {
    kind: DecodedDefaultStatementKindV1,
    definition_origin: DecodedExportDefinitionSourceV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedDefaultStatementKindV1 {
    Expr(Box<DecodedDefaultExpressionV1>),
    InitializationEnsure(DecodedPersistentId<PersistentInitializationUnitId>),
    LocalFunction(DecodedDefaultLocalFunctionV1),
    Return(DecodedOptionalDefaultExpressionV1),
    ValDecl {
        pattern: DecodedDefaultPatternV1,
        init: Box<DecodedDefaultExpressionV1>,
    },
    Assign {
        target: Box<DecodedDefaultAssignTargetV1>,
        value: Box<DecodedDefaultExpressionV1>,
    },
    If {
        condition: Box<DecodedDefaultExpressionV1>,
        then_body: Vec<DecodedDefaultStatementV1>,
        else_body: DecodedOptionalDefaultStatementListV1,
    },
    While {
        condition_setup: Vec<DecodedDefaultStatementV1>,
        condition: Box<DecodedDefaultExpressionV1>,
        body: Vec<DecodedDefaultStatementV1>,
    },
    For(Box<DecodedDefaultForIterationPlanV1>),
    Break,
    Continue,
    When(Box<DecodedDefaultWhenV1>),
    Try(Box<DecodedDefaultTryV1>),
    Throw(Box<DecodedDefaultExpressionV1>),
}

impl DecodedDefaultStatementV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultStatementV1, DefaultStatementResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        let kind = match self.kind {
            DecodedDefaultStatementKindV1::Expr(value) => DefaultStatementKindV1::Expr(Box::new(
                resolve_expression(*value, resolver, locals, 1, 1)?,
            )),
            DecodedDefaultStatementKindV1::InitializationEnsure(unit) => {
                DefaultStatementKindV1::InitializationEnsure(
                    resolver
                        .resolve(unit)
                        .map_err(DefaultStatementResolutionError::InitializationUnit)?,
                )
            }
            DecodedDefaultStatementKindV1::LocalFunction(function) => {
                DefaultStatementKindV1::LocalFunction(
                    function
                        .resolve(resolver, locals)
                        .map_err(DefaultStatementResolutionError::LocalFunction)?,
                )
            }
            DecodedDefaultStatementKindV1::Return(value) => DefaultStatementKindV1::Return(
                resolve_optional_expression(value, resolver, locals, 4, 1)?,
            ),
            DecodedDefaultStatementKindV1::ValDecl { pattern, init } => {
                DefaultStatementKindV1::ValDecl {
                    pattern: pattern
                        .resolve(resolver, locals)
                        .map_err(DefaultStatementResolutionError::Pattern)?,
                    init: Box::new(resolve_expression(*init, resolver, locals, 5, 2)?),
                }
            }
            DecodedDefaultStatementKindV1::Assign { target, value } => {
                DefaultStatementKindV1::Assign {
                    target: Box::new(
                        target
                            .resolve(resolver, locals)
                            .map_err(DefaultStatementResolutionError::AssignTarget)?,
                    ),
                    value: Box::new(resolve_expression(*value, resolver, locals, 6, 2)?),
                }
            }
            DecodedDefaultStatementKindV1::If {
                condition,
                then_body,
                else_body,
            } => {
                require_statement_count(then_body.len(), 7, 2)?;
                DefaultStatementKindV1::If {
                    condition: Box::new(resolve_expression(*condition, resolver, locals, 7, 1)?),
                    then_body: resolve_statements(then_body, resolver, locals, 7, 2)?,
                    else_body: else_body.resolve(resolver, locals).map_err(|error| {
                        DefaultStatementResolutionError::ControlFlow {
                            variant_tag: 7,
                            error,
                        }
                    })?,
                }
            }
            DecodedDefaultStatementKindV1::While {
                condition_setup,
                condition,
                body,
            } => {
                require_statement_count(condition_setup.len(), 8, 1)?;
                require_statement_count(body.len(), 8, 3)?;
                DefaultStatementKindV1::While {
                    condition_setup: resolve_statements(condition_setup, resolver, locals, 8, 1)?,
                    condition: Box::new(resolve_expression(*condition, resolver, locals, 8, 2)?),
                    body: resolve_statements(body, resolver, locals, 8, 3)?,
                }
            }
            DecodedDefaultStatementKindV1::For(plan) => DefaultStatementKindV1::For(Box::new(
                plan.resolve(resolver, locals)
                    .map_err(DefaultStatementResolutionError::For)?,
            )),
            DecodedDefaultStatementKindV1::Break => DefaultStatementKindV1::Break,
            DecodedDefaultStatementKindV1::Continue => DefaultStatementKindV1::Continue,
            DecodedDefaultStatementKindV1::When(value) => {
                DefaultStatementKindV1::When(Box::new(value.resolve(resolver, locals).map_err(
                    |error| DefaultStatementResolutionError::ControlFlow {
                        variant_tag: 12,
                        error,
                    },
                )?))
            }
            DecodedDefaultStatementKindV1::Try(value) => {
                DefaultStatementKindV1::Try(Box::new(value.resolve(resolver, locals).map_err(
                    |error| DefaultStatementResolutionError::ControlFlow {
                        variant_tag: 13,
                        error,
                    },
                )?))
            }
            DecodedDefaultStatementKindV1::Throw(value) => DefaultStatementKindV1::Throw(Box::new(
                resolve_expression(*value, resolver, locals, 14, 1)?,
            )),
        };
        let definition_origin = self
            .definition_origin
            .resolve(resolver)
            .map_err(DefaultStatementResolutionError::DefinitionOrigin)?;
        DefaultStatementV1::try_new(kind, definition_origin)
            .map_err(DefaultStatementResolutionError::Record)
    }
}

impl WireEncode for DecodedDefaultStatementV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.kind.encode(encoder)?;
        encoder.field(2)?;
        self.definition_origin.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultStatementV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            kind: decoder.field(1, DecodedDefaultStatementKindV1::decode)?,
            definition_origin: decoder.field(2, DecodedExportDefinitionSourceV1::decode)?,
        })
    }
}

impl WireEncode for DecodedDefaultStatementKindV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Expr(value) => encode_one(encoder, 1, value.as_ref()),
            Self::InitializationEnsure(unit) => encode_one(encoder, 2, unit),
            Self::LocalFunction(function) => encode_one(encoder, 3, function),
            Self::Return(value) => encode_one(encoder, 4, value),
            Self::ValDecl { pattern, init } => encode_two(encoder, 5, pattern, init.as_ref()),
            Self::Assign { target, value } => {
                encode_two(encoder, 6, target.as_ref(), value.as_ref())
            }
            Self::If {
                condition,
                then_body,
                else_body,
            } => encode_if(encoder, condition.as_ref(), then_body, else_body),
            Self::While {
                condition_setup,
                condition,
                body,
            } => encode_while(encoder, condition_setup, condition.as_ref(), body),
            Self::For(plan) => encode_one(encoder, 9, plan.as_ref()),
            Self::Break => encode_empty(encoder, 10),
            Self::Continue => encode_empty(encoder, 11),
            Self::When(value) => encode_one(encoder, 12, value.as_ref()),
            Self::Try(value) => encode_one(encoder, 13, value.as_ref()),
            Self::Throw(value) => encode_one(encoder, 14, value.as_ref()),
        }
    }
}

impl WireDecode for DecodedDefaultStatementKindV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decode_boxed_expression(decoder, 1).map(Self::Expr)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::InitializationEnsure)
            }
            3 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedDefaultLocalFunctionV1::decode)
                    .map(Self::LocalFunction)
            }
            4 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedOptionalDefaultExpressionV1::decode)
                    .map(Self::Return)
            }
            5 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::ValDecl {
                    pattern: decoder.field(1, DecodedDefaultPatternV1::decode)?,
                    init: decode_boxed_expression(decoder, 2)?,
                })
            }
            6 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Assign {
                    target: decoder
                        .field(1, DecodedDefaultAssignTargetV1::decode)
                        .map(Box::new)?,
                    value: decode_boxed_expression(decoder, 2)?,
                })
            }
            7 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::If {
                    condition: decode_boxed_expression(decoder, 1)?,
                    then_body: decoder.field(2, decode_statements)?,
                    else_body: decoder.field(3, DecodedOptionalDefaultStatementListV1::decode)?,
                })
            }
            8 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::While {
                    condition_setup: decoder.field(1, decode_statements)?,
                    condition: decode_boxed_expression(decoder, 2)?,
                    body: decoder.field(3, decode_statements)?,
                })
            }
            9 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedDefaultForIterationPlanV1::decode)
                    .map(Box::new)
                    .map(Self::For)
            }
            10 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Break)
            }
            11 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Continue)
            }
            12 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedDefaultWhenV1::decode)
                    .map(Box::new)
                    .map(Self::When)
            }
            13 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedDefaultTryV1::decode)
                    .map(Box::new)
                    .map(Self::Try)
            }
            14 => {
                expect_sum_length(decoder, fields, 2)?;
                decode_boxed_expression(decoder, 1).map(Self::Throw)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultStatementResolutionError<E, L> {
    InitializationUnit(E),
    Expression {
        variant_tag: u64,
        field: u32,
        error: DefaultExpressionResolutionError<E, L>,
    },
    LocalFunction(DefaultLocalFunctionResolutionError<E, L>),
    Pattern(DefaultPatternResolutionError<E, L>),
    AssignTarget(DefaultAssignTargetResolutionError<E, L>),
    NestedStatement {
        variant_tag: u64,
        field: u32,
        index: usize,
        error: Box<Self>,
    },
    ControlFlow {
        variant_tag: u64,
        error: DefaultControlFlowResolutionError<E, L>,
    },
    For(DefaultForIterationPlanResolutionError<E, L>),
    DefinitionOrigin(SourceOriginResolutionError<E>),
    Record(DefaultStatementBuildError),
}

impl<E: fmt::Display, L: fmt::Display> fmt::Display for DefaultStatementResolutionError<E, L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InitializationUnit(error) => {
                write!(formatter, "invalid default initialization unit: {error}")
            }
            Self::Expression {
                variant_tag,
                field,
                error,
            } => write!(
                formatter,
                "invalid default statement tag {variant_tag} field {field}: {error}"
            ),
            Self::LocalFunction(error) => {
                write!(
                    formatter,
                    "invalid default local-function statement: {error}"
                )
            }
            Self::Pattern(error) => {
                write!(formatter, "invalid default declaration pattern: {error}")
            }
            Self::AssignTarget(error) => {
                write!(formatter, "invalid default assignment target: {error}")
            }
            Self::NestedStatement {
                variant_tag,
                field,
                index,
                error,
            } => write!(
                formatter,
                "invalid default statement tag {variant_tag} field {field} element {index}: {error}"
            ),
            Self::ControlFlow { variant_tag, error } => write!(
                formatter,
                "invalid default control-flow statement tag {variant_tag}: {error}"
            ),
            Self::For(error) => write!(formatter, "invalid default for statement: {error}"),
            Self::DefinitionOrigin(error) => {
                write!(
                    formatter,
                    "invalid default statement definition origin: {error}"
                )
            }
            Self::Record(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static, L: std::error::Error + 'static> std::error::Error
    for DefaultStatementResolutionError<E, L>
{
}

fn resolve_expression<R, L, E>(
    expression: DecodedDefaultExpressionV1,
    resolver: &mut R,
    locals: &mut L,
    variant_tag: u64,
    field: u32,
) -> Result<crate::DefaultExpressionV1, DefaultStatementResolutionError<E, L::Error>>
where
    R: DefaultStatementReferenceResolver<E>,
    L: TemplateLocalSelectorResolver,
{
    expression.resolve(resolver, locals).map_err(|error| {
        DefaultStatementResolutionError::Expression {
            variant_tag,
            field,
            error,
        }
    })
}

fn resolve_optional_expression<R, L, E>(
    expression: DecodedOptionalDefaultExpressionV1,
    resolver: &mut R,
    locals: &mut L,
    variant_tag: u64,
    field: u32,
) -> Result<OptionalDefaultExpressionV1, DefaultStatementResolutionError<E, L::Error>>
where
    R: DefaultStatementReferenceResolver<E>,
    L: TemplateLocalSelectorResolver,
{
    match expression {
        DecodedOptionalDefaultExpressionV1::Absent => Ok(OptionalDefaultExpressionV1::absent()),
        DecodedOptionalDefaultExpressionV1::Present(value) => {
            resolve_expression(*value, resolver, locals, variant_tag, field)
                .map(OptionalDefaultExpressionV1::present)
        }
    }
}

fn resolve_statements<R, L, E>(
    statements: Vec<DecodedDefaultStatementV1>,
    resolver: &mut R,
    locals: &mut L,
    variant_tag: u64,
    field: u32,
) -> Result<Vec<DefaultStatementV1>, DefaultStatementResolutionError<E, L::Error>>
where
    R: DefaultStatementReferenceResolver<E>,
    L: TemplateLocalSelectorResolver,
{
    let mut resolved = Vec::with_capacity(statements.len());
    for (index, statement) in statements.into_iter().enumerate() {
        resolved.push(statement.resolve(resolver, locals).map_err(|error| {
            DefaultStatementResolutionError::NestedStatement {
                variant_tag,
                field,
                index,
                error: Box::new(error),
            }
        })?);
    }
    Ok(resolved)
}

fn require_statement_count<E, L>(
    length: usize,
    variant_tag: u64,
    field: u32,
) -> Result<(), DefaultStatementResolutionError<E, L>> {
    u32::try_from(length).map(|_| ()).map_err(|_| {
        DefaultStatementResolutionError::Record(DefaultStatementBuildError::TooManyStatements {
            variant_tag,
            field,
        })
    })
}

fn decode_boxed_expression(
    decoder: &mut Decoder<'_>,
    field: u32,
) -> Result<Box<DecodedDefaultExpressionV1>, WireError> {
    decoder
        .field(field, DecodedDefaultExpressionV1::decode)
        .map(Box::new)
}

fn decode_statements(
    decoder: &mut Decoder<'_>,
) -> Result<Vec<DecodedDefaultStatementV1>, WireError> {
    decoder.decode_array(|decoder, _| DecodedDefaultStatementV1::decode(decoder))
}

fn encode_empty(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_one(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_two(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncode,
    second: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    first.encode(encoder)?;
    encoder.field(2)?;
    second.encode(encoder)
}

fn encode_if(
    encoder: &mut Encoder,
    condition: &impl WireEncode,
    then_body: &[impl WireEncode],
    else_body: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(4)?;
    encode_tag(encoder, 7)?;
    encoder.field(1)?;
    condition.encode(encoder)?;
    encoder.field(2)?;
    encode_sequence(encoder, then_body)?;
    encoder.field(3)?;
    else_body.encode(encoder)
}

fn encode_while(
    encoder: &mut Encoder,
    condition_setup: &[impl WireEncode],
    condition: &impl WireEncode,
    body: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(4)?;
    encode_tag(encoder, 8)?;
    encoder.field(1)?;
    encode_sequence(encoder, condition_setup)?;
    encoder.field(2)?;
    condition.encode(encoder)?;
    encoder.field(3)?;
    encode_sequence(encoder, body)
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

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
