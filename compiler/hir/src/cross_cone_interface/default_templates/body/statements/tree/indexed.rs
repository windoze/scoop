use std::fmt;

use scoop_identity::PersistentInitializationUnitId;
use scoop_wire::{Encoder, WireEncode};

use super::{DefaultStatementKindV1, DefaultStatementV1};
use crate::{
    DefaultAssignTargetIndexError, DefaultExpressionIndexError, DefaultLocalFunctionIndexError,
    DefaultPatternIndexError, ExportDefinitionSourceV1, IndexedDefaultAssignTargetV1,
    IndexedDefaultExpressionV1, IndexedDefaultLocalFunctionV1, IndexedDefaultPatternV1,
    OptionalDefaultExpressionV1, TemplateLocalIndexResolver,
};

use super::super::{
    DefaultControlFlowIndexError, DefaultForIterationPlanIndexError,
    IndexedDefaultForIterationPlanV1, IndexedDefaultTryV1, IndexedDefaultWhenV1,
    IndexedOptionalDefaultStatementListV1,
};

#[derive(Debug)]
pub struct IndexedDefaultStatementV1<'a> {
    kind: IndexedDefaultStatementKindV1<'a>,
    definition_origin: &'a ExportDefinitionSourceV1,
}

#[derive(Debug)]
enum IndexedDefaultStatementKindV1<'a> {
    Expr(IndexedDefaultExpressionV1<'a>),
    InitializationEnsure(&'a PersistentInitializationUnitId),
    LocalFunction(IndexedDefaultLocalFunctionV1<'a>),
    Return(IndexedOptionalDefaultExpressionV1<'a>),
    ValDecl {
        pattern: IndexedDefaultPatternV1<'a>,
        init: IndexedDefaultExpressionV1<'a>,
    },
    Assign {
        target: IndexedDefaultAssignTargetV1<'a>,
        value: IndexedDefaultExpressionV1<'a>,
    },
    If {
        condition: IndexedDefaultExpressionV1<'a>,
        then_body: Vec<IndexedDefaultStatementV1<'a>>,
        else_body: IndexedOptionalDefaultStatementListV1<'a>,
    },
    While {
        condition_setup: Vec<IndexedDefaultStatementV1<'a>>,
        condition: IndexedDefaultExpressionV1<'a>,
        body: Vec<IndexedDefaultStatementV1<'a>>,
    },
    For(Box<IndexedDefaultForIterationPlanV1<'a>>),
    Break,
    Continue,
    When(IndexedDefaultWhenV1<'a>),
    Try(IndexedDefaultTryV1<'a>),
    Throw(IndexedDefaultExpressionV1<'a>),
}

#[derive(Debug)]
enum IndexedOptionalDefaultExpressionV1<'a> {
    Absent,
    Present(IndexedDefaultExpressionV1<'a>),
}

impl DefaultStatementV1 {
    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultStatementV1<'_>, DefaultStatementIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        let kind = match &self.kind {
            DefaultStatementKindV1::Expr(value) => {
                IndexedDefaultStatementKindV1::Expr(index_expression(value, resolver, 1, 1)?)
            }
            DefaultStatementKindV1::InitializationEnsure(unit) => {
                IndexedDefaultStatementKindV1::InitializationEnsure(unit)
            }
            DefaultStatementKindV1::LocalFunction(function) => {
                IndexedDefaultStatementKindV1::LocalFunction(
                    function
                        .index_locals(resolver)
                        .map_err(DefaultStatementIndexError::LocalFunction)?,
                )
            }
            DefaultStatementKindV1::Return(value) => IndexedDefaultStatementKindV1::Return(
                index_optional_expression(value, resolver, 4, 1)?,
            ),
            DefaultStatementKindV1::ValDecl { pattern, init } => {
                IndexedDefaultStatementKindV1::ValDecl {
                    pattern: pattern
                        .index_locals(resolver)
                        .map_err(DefaultStatementIndexError::Pattern)?,
                    init: index_expression(init, resolver, 5, 2)?,
                }
            }
            DefaultStatementKindV1::Assign { target, value } => {
                IndexedDefaultStatementKindV1::Assign {
                    target: target
                        .index_locals(resolver)
                        .map_err(DefaultStatementIndexError::AssignTarget)?,
                    value: index_expression(value, resolver, 6, 2)?,
                }
            }
            DefaultStatementKindV1::If {
                condition,
                then_body,
                else_body,
            } => IndexedDefaultStatementKindV1::If {
                condition: index_expression(condition, resolver, 7, 1)?,
                then_body: index_statements(then_body, resolver, 7, 2)?,
                else_body: else_body.index_locals(resolver).map_err(|error| {
                    DefaultStatementIndexError::ControlFlow {
                        variant_tag: 7,
                        error,
                    }
                })?,
            },
            DefaultStatementKindV1::While {
                condition_setup,
                condition,
                body,
            } => IndexedDefaultStatementKindV1::While {
                condition_setup: index_statements(condition_setup, resolver, 8, 1)?,
                condition: index_expression(condition, resolver, 8, 2)?,
                body: index_statements(body, resolver, 8, 3)?,
            },
            DefaultStatementKindV1::For(plan) => IndexedDefaultStatementKindV1::For(Box::new(
                plan.index_locals(resolver)
                    .map_err(DefaultStatementIndexError::For)?,
            )),
            DefaultStatementKindV1::Break => IndexedDefaultStatementKindV1::Break,
            DefaultStatementKindV1::Continue => IndexedDefaultStatementKindV1::Continue,
            DefaultStatementKindV1::When(value) => {
                IndexedDefaultStatementKindV1::When(value.index_locals(resolver).map_err(
                    |error| DefaultStatementIndexError::ControlFlow {
                        variant_tag: 12,
                        error,
                    },
                )?)
            }
            DefaultStatementKindV1::Try(value) => {
                IndexedDefaultStatementKindV1::Try(value.index_locals(resolver).map_err(
                    |error| DefaultStatementIndexError::ControlFlow {
                        variant_tag: 13,
                        error,
                    },
                )?)
            }
            DefaultStatementKindV1::Throw(value) => {
                IndexedDefaultStatementKindV1::Throw(index_expression(value, resolver, 14, 1)?)
            }
        };
        Ok(IndexedDefaultStatementV1 {
            kind,
            definition_origin: &self.definition_origin,
        })
    }
}

impl WireEncode for IndexedDefaultStatementV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.kind.encode(encoder)?;
        encoder.field(2)?;
        self.definition_origin.encode(encoder)
    }
}

impl WireEncode for IndexedDefaultStatementKindV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Expr(value) => encode_one(encoder, 1, value),
            Self::InitializationEnsure(unit) => encode_one(encoder, 2, *unit),
            Self::LocalFunction(function) => encode_one(encoder, 3, function),
            Self::Return(value) => encode_one(encoder, 4, value),
            Self::ValDecl { pattern, init } => encode_two(encoder, 5, pattern, init),
            Self::Assign { target, value } => encode_two(encoder, 6, target, value),
            Self::If {
                condition,
                then_body,
                else_body,
            } => encode_if(encoder, condition, then_body, else_body),
            Self::While {
                condition_setup,
                condition,
                body,
            } => encode_while(encoder, condition_setup, condition, body),
            Self::For(plan) => encode_one(encoder, 9, plan.as_ref()),
            Self::Break => encode_empty(encoder, 10),
            Self::Continue => encode_empty(encoder, 11),
            Self::When(value) => encode_one(encoder, 12, value),
            Self::Try(value) => encode_one(encoder, 13, value),
            Self::Throw(value) => encode_one(encoder, 14, value),
        }
    }
}

impl WireEncode for IndexedOptionalDefaultExpressionV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => encode_empty(encoder, 1),
            Self::Present(value) => encode_one(encoder, 2, value),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultStatementIndexError<E> {
    Expression {
        variant_tag: u64,
        field: u32,
        error: DefaultExpressionIndexError<E>,
    },
    LocalFunction(DefaultLocalFunctionIndexError<E>),
    Pattern(DefaultPatternIndexError<E>),
    AssignTarget(DefaultAssignTargetIndexError<E>),
    NestedStatement {
        variant_tag: u64,
        field: u32,
        index: usize,
        error: Box<Self>,
    },
    ControlFlow {
        variant_tag: u64,
        error: DefaultControlFlowIndexError<E>,
    },
    For(DefaultForIterationPlanIndexError<E>),
}

impl<E: fmt::Display> fmt::Display for DefaultStatementIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expression {
                variant_tag,
                field,
                error,
            } => write!(
                formatter,
                "cannot index default statement tag {variant_tag} field {field}: {error}"
            ),
            Self::LocalFunction(error) => {
                write!(
                    formatter,
                    "cannot index default local-function statement: {error}"
                )
            }
            Self::Pattern(error) => {
                write!(
                    formatter,
                    "cannot index default declaration pattern: {error}"
                )
            }
            Self::AssignTarget(error) => {
                write!(formatter, "cannot index default assignment target: {error}")
            }
            Self::NestedStatement {
                variant_tag,
                field,
                index,
                error,
            } => write!(
                formatter,
                "cannot index default statement tag {variant_tag} field {field} element {index}: {error}"
            ),
            Self::ControlFlow { variant_tag, error } => write!(
                formatter,
                "cannot index default control-flow statement tag {variant_tag}: {error}"
            ),
            Self::For(error) => write!(formatter, "cannot index default for statement: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultStatementIndexError<E> {}

fn index_expression<'a, I>(
    expression: &'a crate::DefaultExpressionV1,
    resolver: &mut I,
    variant_tag: u64,
    field: u32,
) -> Result<IndexedDefaultExpressionV1<'a>, DefaultStatementIndexError<I::Error>>
where
    I: TemplateLocalIndexResolver,
{
    expression
        .index_locals(resolver)
        .map_err(|error| DefaultStatementIndexError::Expression {
            variant_tag,
            field,
            error,
        })
}

fn index_optional_expression<'a, I>(
    expression: &'a OptionalDefaultExpressionV1,
    resolver: &mut I,
    variant_tag: u64,
    field: u32,
) -> Result<IndexedOptionalDefaultExpressionV1<'a>, DefaultStatementIndexError<I::Error>>
where
    I: TemplateLocalIndexResolver,
{
    match expression {
        OptionalDefaultExpressionV1::Absent => Ok(IndexedOptionalDefaultExpressionV1::Absent),
        OptionalDefaultExpressionV1::Present(value) => {
            index_expression(value, resolver, variant_tag, field)
                .map(IndexedOptionalDefaultExpressionV1::Present)
        }
    }
}

fn index_statements<'a, I>(
    statements: &'a [DefaultStatementV1],
    resolver: &mut I,
    variant_tag: u64,
    field: u32,
) -> Result<Vec<IndexedDefaultStatementV1<'a>>, DefaultStatementIndexError<I::Error>>
where
    I: TemplateLocalIndexResolver,
{
    let mut indexed = Vec::with_capacity(statements.len());
    for (index, statement) in statements.iter().enumerate() {
        indexed.push(statement.index_locals(resolver).map_err(|error| {
            DefaultStatementIndexError::NestedStatement {
                variant_tag,
                field,
                index,
                error: Box::new(error),
            }
        })?);
    }
    Ok(indexed)
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
