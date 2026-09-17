use std::fmt;
use std::num::NonZeroU32;

use scoop_identity::SignatureTypeKey;
use scoop_wire::{Encoder, WireEncode};

use super::{
    DefaultAppliedOptionV1, DefaultBindingActionV1, DefaultBindingActionViewV1,
    DefaultBindingPlanV1, DefaultForIterationPlanV1, DefaultIteratorConformanceV1,
    DefaultIteratorNextV1,
};
use crate::{
    DefaultBindingLeafIndexError, DefaultBindingProjectionV1, DefaultBindingShapeIndexError,
    DefaultBindingTemporaryIndexError, DefaultCallableRefV1, DefaultEnumVariantFieldRefV1,
    DefaultEnumVariantRefV1, DefaultExpressionIndexError, ExportDefinitionSourceV1,
    IndexedDefaultBindingLeafV1, IndexedDefaultBindingShapeV1, IndexedDefaultBindingTemporaryV1,
    IndexedDefaultExpressionV1, TemplateLocalIndexResolver,
};

use super::super::{DefaultStatementIndexError, DefaultStatementV1, IndexedDefaultStatementV1};

#[derive(Debug)]
pub struct IndexedDefaultBindingActionV1<'a>(IndexedDefaultBindingActionKindV1<'a>);

#[derive(Debug)]
enum IndexedDefaultBindingActionKindV1<'a> {
    Project {
        source: IndexedDefaultBindingTemporaryV1<'a>,
        result: IndexedDefaultBindingTemporaryV1<'a>,
        projection: &'a DefaultBindingProjectionV1,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    Component {
        source: IndexedDefaultBindingTemporaryV1<'a>,
        index: NonZeroU32,
        result: IndexedDefaultBindingTemporaryV1<'a>,
        setup: Vec<IndexedDefaultStatementV1<'a>>,
        call: IndexedDefaultExpressionV1<'a>,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
    Bind {
        source: IndexedDefaultBindingTemporaryV1<'a>,
        target: IndexedDefaultBindingLeafV1<'a>,
        definition_origin: &'a ExportDefinitionSourceV1,
    },
}

#[derive(Debug)]
pub struct IndexedDefaultBindingPlanV1<'a> {
    subject: IndexedDefaultBindingTemporaryV1<'a>,
    shape: IndexedDefaultBindingShapeV1<'a>,
    actions: Vec<IndexedDefaultBindingActionV1<'a>>,
}

#[derive(Debug)]
pub struct IndexedDefaultIteratorConformanceV1<'a> {
    source: IndexedDefaultBindingTemporaryV1<'a>,
    iterator: IndexedDefaultBindingTemporaryV1<'a>,
    interface_type: &'a SignatureTypeKey,
    definition_origin: &'a ExportDefinitionSourceV1,
}

#[derive(Debug)]
pub struct IndexedDefaultAppliedOptionV1<'a> {
    some_payload: &'a DefaultEnumVariantFieldRefV1,
    none: &'a DefaultEnumVariantRefV1,
}

#[derive(Debug)]
pub struct IndexedDefaultIteratorNextV1<'a> {
    callable: &'a DefaultCallableRefV1,
    result: IndexedDefaultBindingTemporaryV1<'a>,
    option: IndexedDefaultAppliedOptionV1<'a>,
    element: IndexedDefaultBindingTemporaryV1<'a>,
    definition_origin: &'a ExportDefinitionSourceV1,
}

#[derive(Debug)]
pub struct IndexedDefaultForIterationPlanV1<'a> {
    source_setup: Vec<IndexedDefaultStatementV1<'a>>,
    source: IndexedDefaultBindingTemporaryV1<'a>,
    source_init: IndexedDefaultExpressionV1<'a>,
    iterator_setup: Vec<IndexedDefaultStatementV1<'a>>,
    iterator_call: IndexedDefaultExpressionV1<'a>,
    conformance: IndexedDefaultIteratorConformanceV1<'a>,
    next: IndexedDefaultIteratorNextV1<'a>,
    binding: IndexedDefaultBindingPlanV1<'a>,
    body: Vec<IndexedDefaultStatementV1<'a>>,
}

impl DefaultBindingActionV1 {
    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultBindingActionV1<'_>, DefaultForIterationPlanIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        self.index_locals_at(resolver, 0)
    }

    fn index_locals_at<I>(
        &self,
        resolver: &mut I,
        action_index: usize,
    ) -> Result<IndexedDefaultBindingActionV1<'_>, DefaultForIterationPlanIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        let kind = match self.view() {
            DefaultBindingActionViewV1::Project {
                source,
                result,
                projection,
                definition_origin,
            } => IndexedDefaultBindingActionKindV1::Project {
                source: index_temporary(source, resolver, "binding project source")?,
                result: index_temporary(result, resolver, "binding project result")?,
                projection,
                definition_origin,
            },
            DefaultBindingActionViewV1::Component {
                source,
                index,
                result,
                setup,
                call,
                definition_origin,
            } => IndexedDefaultBindingActionKindV1::Component {
                source: index_temporary(source, resolver, "binding component source")?,
                index,
                result: index_temporary(result, resolver, "binding component result")?,
                setup: index_statements(
                    setup,
                    resolver,
                    StatementContext::BindingComponent { action_index },
                )?,
                call: call.index_locals(resolver).map_err(|error| {
                    DefaultForIterationPlanIndexError::Expression {
                        context: "binding component call",
                        error,
                    }
                })?,
                definition_origin,
            },
            DefaultBindingActionViewV1::Bind {
                source,
                target,
                definition_origin,
            } => IndexedDefaultBindingActionKindV1::Bind {
                source: index_temporary(source, resolver, "binding bind source")?,
                target: target.index_local(resolver).map_err(|error| {
                    DefaultForIterationPlanIndexError::BindingLeaf {
                        action_index,
                        error,
                    }
                })?,
                definition_origin,
            },
        };
        Ok(IndexedDefaultBindingActionV1(kind))
    }
}

impl DefaultBindingPlanV1 {
    fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultBindingPlanV1<'_>, DefaultForIterationPlanIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        let mut actions = Vec::with_capacity(self.actions.len());
        for (index, action) in self.actions.iter().enumerate() {
            actions.push(action.index_locals_at(resolver, index)?);
        }
        Ok(IndexedDefaultBindingPlanV1 {
            subject: index_temporary(&self.subject, resolver, "binding subject")?,
            shape: self
                .shape
                .index_locals(resolver)
                .map_err(DefaultForIterationPlanIndexError::BindingShape)?,
            actions,
        })
    }
}

impl DefaultIteratorConformanceV1 {
    fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultIteratorConformanceV1<'_>, DefaultForIterationPlanIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        Ok(IndexedDefaultIteratorConformanceV1 {
            source: index_temporary(&self.source, resolver, "iterator conformance source")?,
            iterator: index_temporary(&self.iterator, resolver, "iterator conformance result")?,
            interface_type: &self.interface_type,
            definition_origin: &self.definition_origin,
        })
    }
}

impl DefaultAppliedOptionV1 {
    fn indexed(&self) -> IndexedDefaultAppliedOptionV1<'_> {
        IndexedDefaultAppliedOptionV1 {
            some_payload: &self.some_payload,
            none: &self.none,
        }
    }
}

impl DefaultIteratorNextV1 {
    fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultIteratorNextV1<'_>, DefaultForIterationPlanIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        Ok(IndexedDefaultIteratorNextV1 {
            callable: &self.callable,
            result: index_temporary(&self.result, resolver, "iterator next result")?,
            option: self.option.indexed(),
            element: index_temporary(&self.element, resolver, "iterator next element")?,
            definition_origin: &self.definition_origin,
        })
    }
}

impl DefaultForIterationPlanV1 {
    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultForIterationPlanV1<'_>, DefaultForIterationPlanIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        Ok(IndexedDefaultForIterationPlanV1 {
            source_setup: index_statements(
                &self.source_setup,
                resolver,
                StatementContext::SourceSetup,
            )?,
            source: index_temporary(&self.source, resolver, "for source")?,
            source_init: self.source_init.index_locals(resolver).map_err(|error| {
                DefaultForIterationPlanIndexError::Expression {
                    context: "for source init",
                    error,
                }
            })?,
            iterator_setup: index_statements(
                &self.iterator_setup,
                resolver,
                StatementContext::IteratorSetup,
            )?,
            iterator_call: self.iterator_call.index_locals(resolver).map_err(|error| {
                DefaultForIterationPlanIndexError::Expression {
                    context: "for iterator call",
                    error,
                }
            })?,
            conformance: self.conformance.index_locals(resolver)?,
            next: self.next.index_locals(resolver)?,
            binding: self.binding.index_locals(resolver)?,
            body: index_statements(&self.body, resolver, StatementContext::Body)?,
        })
    }
}

impl WireEncode for IndexedDefaultBindingActionV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match &self.0 {
            IndexedDefaultBindingActionKindV1::Project {
                source,
                result,
                projection,
                definition_origin,
            } => encode_project(encoder, source, result, *projection, *definition_origin),
            IndexedDefaultBindingActionKindV1::Component {
                source,
                index,
                result,
                setup,
                call,
                definition_origin,
            } => encode_component(
                encoder,
                source,
                *index,
                result,
                setup,
                call,
                *definition_origin,
            ),
            IndexedDefaultBindingActionKindV1::Bind {
                source,
                target,
                definition_origin,
            } => encode_bind(encoder, source, target, *definition_origin),
        }
    }
}

impl WireEncode for IndexedDefaultBindingPlanV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.subject.encode(encoder)?;
        encoder.field(2)?;
        self.shape.encode(encoder)?;
        encoder.field(3)?;
        encode_sequence(encoder, &self.actions)
    }
}

impl WireEncode for IndexedDefaultIteratorConformanceV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.source.encode(encoder)?;
        encoder.field(2)?;
        self.iterator.encode(encoder)?;
        encoder.field(3)?;
        self.interface_type.encode(encoder)?;
        encoder.field(4)?;
        self.definition_origin.encode(encoder)
    }
}

impl WireEncode for IndexedDefaultAppliedOptionV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.some_payload.encode(encoder)?;
        encoder.field(2)?;
        self.none.encode(encoder)
    }
}

impl WireEncode for IndexedDefaultIteratorNextV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.callable.encode(encoder)?;
        encoder.field(2)?;
        self.result.encode(encoder)?;
        encoder.field(3)?;
        self.option.encode(encoder)?;
        encoder.field(4)?;
        self.element.encode(encoder)?;
        encoder.field(5)?;
        self.definition_origin.encode(encoder)
    }
}

impl WireEncode for IndexedDefaultForIterationPlanV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encoder.field(1)?;
        encode_sequence(encoder, &self.source_setup)?;
        encoder.field(2)?;
        self.source.encode(encoder)?;
        encoder.field(3)?;
        self.source_init.encode(encoder)?;
        encoder.field(4)?;
        encode_sequence(encoder, &self.iterator_setup)?;
        encoder.field(5)?;
        self.iterator_call.encode(encoder)?;
        encoder.field(6)?;
        self.conformance.encode(encoder)?;
        encoder.field(7)?;
        self.next.encode(encoder)?;
        encoder.field(8)?;
        self.binding.encode(encoder)?;
        encoder.field(9)?;
        encode_sequence(encoder, &self.body)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultForIterationPlanIndexError<E> {
    Temporary {
        context: &'static str,
        error: DefaultBindingTemporaryIndexError<E>,
    },
    BindingLeaf {
        action_index: usize,
        error: DefaultBindingLeafIndexError<E>,
    },
    BindingShape(DefaultBindingShapeIndexError<E>),
    Expression {
        context: &'static str,
        error: DefaultExpressionIndexError<E>,
    },
    Statement {
        context: StatementContext,
        index: usize,
        error: Box<DefaultStatementIndexError<E>>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatementContext {
    SourceSetup,
    IteratorSetup,
    BindingComponent { action_index: usize },
    Body,
}

impl fmt::Display for StatementContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceSetup => formatter.write_str("source setup"),
            Self::IteratorSetup => formatter.write_str("iterator setup"),
            Self::BindingComponent { action_index } => {
                write!(formatter, "binding action {action_index} component setup")
            }
            Self::Body => formatter.write_str("body"),
        }
    }
}

impl<E: fmt::Display> fmt::Display for DefaultForIterationPlanIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Temporary { context, error } => {
                write!(formatter, "cannot index default {context}: {error}")
            }
            Self::BindingLeaf {
                action_index,
                error,
            } => write!(
                formatter,
                "cannot index default binding action {action_index} target: {error}"
            ),
            Self::BindingShape(error) => {
                write!(formatter, "cannot index default binding shape: {error}")
            }
            Self::Expression { context, error } => {
                write!(formatter, "cannot index default {context}: {error}")
            }
            Self::Statement {
                context,
                index,
                error,
            } => write!(
                formatter,
                "cannot index default for {context} statement {index}: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultForIterationPlanIndexError<E> {}

fn index_temporary<'a, I>(
    temporary: &'a crate::DefaultBindingTemporaryV1,
    resolver: &mut I,
    context: &'static str,
) -> Result<IndexedDefaultBindingTemporaryV1<'a>, DefaultForIterationPlanIndexError<I::Error>>
where
    I: TemplateLocalIndexResolver,
{
    temporary
        .index_local(resolver)
        .map_err(|error| DefaultForIterationPlanIndexError::Temporary { context, error })
}

fn index_statements<'a, I>(
    statements: &'a [DefaultStatementV1],
    resolver: &mut I,
    context: StatementContext,
) -> Result<Vec<IndexedDefaultStatementV1<'a>>, DefaultForIterationPlanIndexError<I::Error>>
where
    I: TemplateLocalIndexResolver,
{
    let mut indexed = Vec::with_capacity(statements.len());
    for (index, statement) in statements.iter().enumerate() {
        indexed.push(statement.index_locals(resolver).map_err(|error| {
            DefaultForIterationPlanIndexError::Statement {
                context,
                index,
                error: Box::new(error),
            }
        })?);
    }
    Ok(indexed)
}

fn encode_project(
    encoder: &mut Encoder,
    source: &impl WireEncode,
    result: &impl WireEncode,
    projection: &impl WireEncode,
    definition_origin: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(5)?;
    encode_tag(encoder, 1)?;
    encoder.field(1)?;
    source.encode(encoder)?;
    encoder.field(2)?;
    result.encode(encoder)?;
    encoder.field(3)?;
    projection.encode(encoder)?;
    encoder.field(4)?;
    definition_origin.encode(encoder)
}

fn encode_component(
    encoder: &mut Encoder,
    source: &impl WireEncode,
    index: NonZeroU32,
    result: &impl WireEncode,
    setup: &[impl WireEncode],
    call: &impl WireEncode,
    definition_origin: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(7)?;
    encode_tag(encoder, 2)?;
    encoder.field(1)?;
    source.encode(encoder)?;
    encoder.field(2)?;
    encoder.unsigned(u64::from(index.get()))?;
    encoder.field(3)?;
    result.encode(encoder)?;
    encoder.field(4)?;
    encode_sequence(encoder, setup)?;
    encoder.field(5)?;
    call.encode(encoder)?;
    encoder.field(6)?;
    definition_origin.encode(encoder)
}

fn encode_bind(
    encoder: &mut Encoder,
    source: &impl WireEncode,
    target: &impl WireEncode,
    definition_origin: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(4)?;
    encode_tag(encoder, 3)?;
    encoder.field(1)?;
    source.encode(encoder)?;
    encoder.field(2)?;
    target.encode(encoder)?;
    encoder.field(3)?;
    definition_origin.encode(encoder)
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
