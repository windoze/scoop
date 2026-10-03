use std::fmt;

use scoop_identity::SignatureTypeKey;
use scoop_wire::{Encoder, WireEncode};

use super::{
    DefaultCatchV1, DefaultTryV1, DefaultWhenArmV1, DefaultWhenFallbackV1,
    DefaultWhenFallbackViewV1, DefaultWhenGuardV1, DefaultWhenV1, OptionalDefaultStatementListV1,
    OptionalDefaultStatementListViewV1, OptionalDefaultWhenGuardV1,
};
use crate::{
    DefaultExpressionIndexError, DefaultPatternIndexError, IndexedDefaultExpressionV1,
    IndexedDefaultPatternV1, TemplateLocalIndexResolver,
};

use super::super::{DefaultStatementIndexError, DefaultStatementV1, IndexedDefaultStatementV1};

#[derive(Debug)]
pub struct IndexedDefaultWhenV1<'a> {
    subject: IndexedDefaultExpressionV1<'a>,
    arms: Vec<IndexedDefaultWhenArmV1<'a>>,
    fallback: IndexedDefaultWhenFallbackV1<'a>,
}

#[derive(Debug)]
pub struct IndexedDefaultWhenArmV1<'a> {
    pattern: IndexedDefaultPatternV1<'a>,
    guard: IndexedOptionalDefaultWhenGuardV1<'a>,
    body: Vec<IndexedDefaultStatementV1<'a>>,
    definition_origin: &'a crate::ExportDefinitionSourceV1,
}

#[derive(Debug)]
pub enum IndexedOptionalDefaultWhenGuardV1<'a> {
    Absent,
    Present(IndexedDefaultWhenGuardV1<'a>),
}

#[derive(Debug)]
pub struct IndexedDefaultWhenGuardV1<'a> {
    setup: Vec<IndexedDefaultStatementV1<'a>>,
    condition: IndexedDefaultExpressionV1<'a>,
}

#[derive(Debug)]
pub enum IndexedDefaultWhenFallbackV1<'a> {
    Else(Vec<IndexedDefaultStatementV1<'a>>),
    IrrefutableArm {
        subject_type: &'a SignatureTypeKey,
    },
    PatternMatrix {
        subject_type: &'a SignatureTypeKey,
    },
    EnumPatternMatrix {
        subject_type: &'a SignatureTypeKey,
        owner_type: &'a SignatureTypeKey,
    },
}

#[derive(Debug)]
pub struct IndexedDefaultTryV1<'a> {
    body: Vec<IndexedDefaultStatementV1<'a>>,
    catches: Vec<IndexedDefaultCatchV1<'a>>,
    finally_body: IndexedOptionalDefaultStatementListV1<'a>,
}

#[derive(Debug)]
pub struct IndexedDefaultCatchV1<'a> {
    local_index: u32,
    value_type: &'a SignatureTypeKey,
    body: Vec<IndexedDefaultStatementV1<'a>>,
    definition_origin: &'a crate::ExportDefinitionSourceV1,
}

#[derive(Debug)]
pub enum IndexedOptionalDefaultStatementListV1<'a> {
    Absent,
    Present(Vec<IndexedDefaultStatementV1<'a>>),
}

impl DefaultWhenV1 {
    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultWhenV1<'_>, DefaultControlFlowIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        let subject = self.subject.index_locals(resolver).map_err(|error| {
            DefaultControlFlowIndexError::Expression {
                context: "when subject",
                error,
            }
        })?;
        let mut arms = Vec::with_capacity(self.arms.len());
        for (index, arm) in self.arms.iter().enumerate() {
            arms.push(arm.index_locals(resolver, index)?);
        }
        Ok(IndexedDefaultWhenV1 {
            subject,
            arms,
            fallback: self.fallback.index_locals(resolver)?,
        })
    }
}

impl DefaultWhenArmV1 {
    fn index_locals<I>(
        &self,
        resolver: &mut I,
        arm_index: usize,
    ) -> Result<IndexedDefaultWhenArmV1<'_>, DefaultControlFlowIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        Ok(IndexedDefaultWhenArmV1 {
            pattern: self
                .pattern
                .index_locals(resolver)
                .map_err(|error| DefaultControlFlowIndexError::Pattern { arm_index, error })?,
            guard: self.guard.index_locals(resolver)?,
            body: index_statements(&self.body, resolver, "when arm body")?,
            definition_origin: &self.definition_origin,
        })
    }
}

impl OptionalDefaultWhenGuardV1 {
    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedOptionalDefaultWhenGuardV1<'_>, DefaultControlFlowIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        match self {
            Self::Absent => Ok(IndexedOptionalDefaultWhenGuardV1::Absent),
            Self::Present(guard) => guard
                .index_locals(resolver)
                .map(IndexedOptionalDefaultWhenGuardV1::Present),
        }
    }
}

impl DefaultWhenGuardV1 {
    fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultWhenGuardV1<'_>, DefaultControlFlowIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        Ok(IndexedDefaultWhenGuardV1 {
            setup: index_statements(&self.setup, resolver, "when guard setup")?,
            condition: self.condition.index_locals(resolver).map_err(|error| {
                DefaultControlFlowIndexError::Expression {
                    context: "when guard condition",
                    error,
                }
            })?,
        })
    }
}

impl DefaultWhenFallbackV1 {
    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultWhenFallbackV1<'_>, DefaultControlFlowIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        match self.view() {
            DefaultWhenFallbackViewV1::Else(statements) => {
                index_statements(statements, resolver, "when else")
                    .map(IndexedDefaultWhenFallbackV1::Else)
            }
            DefaultWhenFallbackViewV1::IrrefutableArm { subject_type } => {
                Ok(IndexedDefaultWhenFallbackV1::IrrefutableArm { subject_type })
            }
            DefaultWhenFallbackViewV1::PatternMatrix { subject_type } => {
                Ok(IndexedDefaultWhenFallbackV1::PatternMatrix { subject_type })
            }
            DefaultWhenFallbackViewV1::EnumPatternMatrix {
                subject_type,
                owner_type,
            } => Ok(IndexedDefaultWhenFallbackV1::EnumPatternMatrix {
                subject_type,
                owner_type,
            }),
        }
    }
}

impl DefaultTryV1 {
    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultTryV1<'_>, DefaultControlFlowIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        let mut catches = Vec::with_capacity(self.catches.len());
        for (index, catch) in self.catches.iter().enumerate() {
            catches.push(catch.index_locals(resolver, index)?);
        }
        Ok(IndexedDefaultTryV1 {
            body: index_statements(&self.body, resolver, "try body")?,
            catches,
            finally_body: self.finally_body.index_locals(resolver)?,
        })
    }
}

impl DefaultCatchV1 {
    fn index_locals<I>(
        &self,
        resolver: &mut I,
        catch_index: usize,
    ) -> Result<IndexedDefaultCatchV1<'_>, DefaultControlFlowIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        Ok(IndexedDefaultCatchV1 {
            local_index: resolver
                .resolve_template_local_index(&self.local)
                .map_err(|error| DefaultControlFlowIndexError::CatchLocal { catch_index, error })?,
            value_type: &self.value_type,
            body: index_statements(&self.body, resolver, "catch body")?,
            definition_origin: &self.definition_origin,
        })
    }
}

impl OptionalDefaultStatementListV1 {
    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedOptionalDefaultStatementListV1<'_>, DefaultControlFlowIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        match self.view() {
            OptionalDefaultStatementListViewV1::Absent => {
                Ok(IndexedOptionalDefaultStatementListV1::Absent)
            }
            OptionalDefaultStatementListViewV1::Present(statements) => {
                index_statements(statements, resolver, "optional body")
                    .map(IndexedOptionalDefaultStatementListV1::Present)
            }
        }
    }
}

impl WireEncode for IndexedDefaultWhenV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.subject.encode(encoder)?;
        encoder.field(2)?;
        encode_sequence(encoder, &self.arms)?;
        encoder.field(3)?;
        self.fallback.encode(encoder)
    }
}

impl WireEncode for IndexedDefaultWhenArmV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.pattern.encode(encoder)?;
        encoder.field(2)?;
        self.guard.encode(encoder)?;
        encoder.field(3)?;
        encode_sequence(encoder, &self.body)?;
        encoder.field(4)?;
        self.definition_origin.encode(encoder)
    }
}

impl WireEncode for IndexedOptionalDefaultWhenGuardV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => encode_empty(encoder, 1),
            Self::Present(guard) => encode_one(encoder, 2, guard),
        }
    }
}

impl WireEncode for IndexedDefaultWhenGuardV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encode_sequence(encoder, &self.setup)?;
        encoder.field(2)?;
        self.condition.encode(encoder)
    }
}

impl WireEncode for IndexedDefaultWhenFallbackV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Else(statements) => encode_one(encoder, 1, &WireSequence(statements)),
            Self::IrrefutableArm { subject_type } => encode_one(encoder, 2, *subject_type),
            Self::PatternMatrix { subject_type } => encode_one(encoder, 3, *subject_type),
            Self::EnumPatternMatrix {
                subject_type,
                owner_type,
            } => encode_two(encoder, 4, *subject_type, *owner_type),
        }
    }
}

impl WireEncode for IndexedDefaultTryV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encode_sequence(encoder, &self.body)?;
        encoder.field(2)?;
        encode_sequence(encoder, &self.catches)?;
        encoder.field(3)?;
        self.finally_body.encode(encoder)
    }
}

impl WireEncode for IndexedDefaultCatchV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.local_index))?;
        encoder.field(2)?;
        self.value_type.encode(encoder)?;
        encoder.field(3)?;
        encode_sequence(encoder, &self.body)?;
        encoder.field(4)?;
        self.definition_origin.encode(encoder)
    }
}

impl WireEncode for IndexedOptionalDefaultStatementListV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => encode_empty(encoder, 1),
            Self::Present(statements) => encode_one(encoder, 2, &WireSequence(statements)),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultControlFlowIndexError<E> {
    Expression {
        context: &'static str,
        error: DefaultExpressionIndexError<E>,
    },
    Pattern {
        arm_index: usize,
        error: DefaultPatternIndexError<E>,
    },
    CatchLocal {
        catch_index: usize,
        error: E,
    },
    Statement {
        context: &'static str,
        index: usize,
        error: Box<DefaultStatementIndexError<E>>,
    },
}

impl<E: fmt::Display> fmt::Display for DefaultControlFlowIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expression { context, error } => {
                write!(formatter, "cannot index default {context}: {error}")
            }
            Self::Pattern { arm_index, error } => {
                write!(
                    formatter,
                    "cannot index default when arm {arm_index}: {error}"
                )
            }
            Self::CatchLocal { catch_index, error } => {
                write!(
                    formatter,
                    "cannot index default catch {catch_index} local: {error}"
                )
            }
            Self::Statement {
                context,
                index,
                error,
            } => write!(
                formatter,
                "cannot index default {context} statement {index}: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultControlFlowIndexError<E> {}

fn index_statements<'a, I>(
    statements: &'a [DefaultStatementV1],
    resolver: &mut I,
    context: &'static str,
) -> Result<Vec<IndexedDefaultStatementV1<'a>>, DefaultControlFlowIndexError<I::Error>>
where
    I: TemplateLocalIndexResolver,
{
    let mut indexed = Vec::with_capacity(statements.len());
    for (index, statement) in statements.iter().enumerate() {
        indexed.push(statement.index_locals(resolver).map_err(|error| {
            DefaultControlFlowIndexError::Statement {
                context,
                index,
                error: Box::new(error),
            }
        })?);
    }
    Ok(indexed)
}

struct WireSequence<'a, T>(&'a [T]);

impl<T: WireEncode> WireEncode for WireSequence<'_, T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_sequence(encoder, self.0)
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

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}
