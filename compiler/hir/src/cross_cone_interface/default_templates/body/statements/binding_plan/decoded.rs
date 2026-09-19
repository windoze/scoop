mod resolution_nodes;

use std::fmt;
use std::num::NonZeroU32;

use scoop_identity::{DecodedSignatureTypeKey, SourceOriginResolutionError};

use super::{
    DefaultAppliedOptionV1, DefaultBindingActionBuildError, DefaultBindingActionV1,
    DefaultBindingPlanBuildError, DefaultBindingPlanV1, DefaultForIterationPlanBuildError,
    DefaultForIterationPlanV1, DefaultIteratorConformanceV1, DefaultIteratorNextV1,
};
use crate::{
    DecodedDefaultBindingLeafV1, DecodedDefaultBindingProjectionV1, DecodedDefaultBindingShapeV1,
    DecodedDefaultBindingTemporaryV1, DecodedDefaultCallableRefV1,
    DecodedDefaultEnumVariantFieldRefV1, DecodedDefaultEnumVariantRefV1,
    DecodedDefaultExpressionV1, DecodedExportDefinitionSourceV1, DefaultBindingLeafResolutionError,
    DefaultBindingProjectionResolutionError, DefaultBindingShapeResolutionError,
    DefaultBindingTemporaryResolutionError, DefaultCallableRefResolutionError,
    DefaultEnumVariantFieldRefResolutionError, DefaultEnumVariantRefResolutionError,
    DefaultExpressionResolutionError, TemplateLocalSelectorResolver,
};

use super::super::{
    DecodedDefaultStatementV1, DefaultStatementReferenceResolver, DefaultStatementResolutionError,
    DefaultStatementV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDefaultBindingActionV1 {
    Project {
        source: DecodedDefaultBindingTemporaryV1,
        result: DecodedDefaultBindingTemporaryV1,
        projection: DecodedDefaultBindingProjectionV1,
        definition_origin: DecodedExportDefinitionSourceV1,
    },
    Component {
        source: DecodedDefaultBindingTemporaryV1,
        index: NonZeroU32,
        result: DecodedDefaultBindingTemporaryV1,
        setup: Vec<DecodedDefaultStatementV1>,
        call: Box<DecodedDefaultExpressionV1>,
        definition_origin: DecodedExportDefinitionSourceV1,
    },
    Bind {
        source: DecodedDefaultBindingTemporaryV1,
        target: DecodedDefaultBindingLeafV1,
        definition_origin: DecodedExportDefinitionSourceV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultBindingPlanV1 {
    subject: DecodedDefaultBindingTemporaryV1,
    shape: DecodedDefaultBindingShapeV1,
    actions: Vec<DecodedDefaultBindingActionV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultIteratorConformanceV1 {
    source: DecodedDefaultBindingTemporaryV1,
    iterator: DecodedDefaultBindingTemporaryV1,
    interface_type: DecodedSignatureTypeKey,
    definition_origin: DecodedExportDefinitionSourceV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultAppliedOptionV1 {
    some_payload: DecodedDefaultEnumVariantFieldRefV1,
    none: DecodedDefaultEnumVariantRefV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultIteratorNextV1 {
    callable: DecodedDefaultCallableRefV1,
    result: DecodedDefaultBindingTemporaryV1,
    option: DecodedDefaultAppliedOptionV1,
    element: DecodedDefaultBindingTemporaryV1,
    definition_origin: DecodedExportDefinitionSourceV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultForIterationPlanV1 {
    source_setup: Vec<DecodedDefaultStatementV1>,
    source: DecodedDefaultBindingTemporaryV1,
    source_init: Box<DecodedDefaultExpressionV1>,
    iterator_setup: Vec<DecodedDefaultStatementV1>,
    iterator_call: Box<DecodedDefaultExpressionV1>,
    conformance: DecodedDefaultIteratorConformanceV1,
    next: DecodedDefaultIteratorNextV1,
    binding: DecodedDefaultBindingPlanV1,
    body: Vec<DecodedDefaultStatementV1>,
}

impl DecodedDefaultBindingActionV1 {
    fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
        action_index: usize,
    ) -> Result<DefaultBindingActionV1, DefaultForIterationPlanResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        match self {
            Self::Project {
                source,
                result,
                projection,
                definition_origin,
            } => Ok(DefaultBindingActionV1::project(
                resolve_temporary(source, resolver, locals, "binding project source")?,
                resolve_temporary(result, resolver, locals, "binding project result")?,
                projection.resolve(resolver).map_err(|error| {
                    DefaultForIterationPlanResolutionError::Projection {
                        action_index,
                        error,
                    }
                })?,
                resolve_action_origin(definition_origin, resolver, action_index)?,
            )),
            Self::Component {
                source,
                index,
                result,
                setup,
                call,
                definition_origin,
            } => {
                require_action_setup(setup.len())?;
                let source =
                    resolve_temporary(source, resolver, locals, "binding component source")?;
                let result =
                    resolve_temporary(result, resolver, locals, "binding component result")?;
                let setup = resolve_statements(
                    setup,
                    resolver,
                    locals,
                    StatementContext::BindingComponent { action_index },
                )?;
                let call = call.resolve(resolver, locals).map_err(|error| {
                    DefaultForIterationPlanResolutionError::Expression {
                        context: "binding component call",
                        error,
                    }
                })?;
                let definition_origin =
                    resolve_action_origin(definition_origin, resolver, action_index)?;
                DefaultBindingActionV1::try_component(
                    source,
                    index,
                    result,
                    setup,
                    call,
                    definition_origin,
                )
                .map_err(DefaultForIterationPlanResolutionError::ActionShape)
            }
            Self::Bind {
                source,
                target,
                definition_origin,
            } => Ok(DefaultBindingActionV1::bind(
                resolve_temporary(source, resolver, locals, "binding bind source")?,
                target.resolve(resolver, locals).map_err(|error| {
                    DefaultForIterationPlanResolutionError::BindingLeaf {
                        action_index,
                        error,
                    }
                })?,
                resolve_action_origin(definition_origin, resolver, action_index)?,
            )),
        }
    }
}

impl DecodedDefaultBindingPlanV1 {
    fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultBindingPlanV1, DefaultForIterationPlanResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        require_action_count(self.actions.len())?;
        let subject = resolve_temporary(self.subject, resolver, locals, "binding subject")?;
        let shape = self
            .shape
            .resolve(resolver, locals)
            .map_err(DefaultForIterationPlanResolutionError::BindingShape)?;
        let mut actions = Vec::with_capacity(self.actions.len());
        for (index, action) in self.actions.into_iter().enumerate() {
            actions.push(action.resolve(resolver, locals, index)?);
        }
        DefaultBindingPlanV1::try_new(subject, shape, actions)
            .map_err(DefaultForIterationPlanResolutionError::BindingPlanShape)
    }
}

impl DecodedDefaultIteratorConformanceV1 {
    fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultIteratorConformanceV1, DefaultForIterationPlanResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        Ok(DefaultIteratorConformanceV1::new(
            resolve_temporary(self.source, resolver, locals, "iterator conformance source")?,
            resolve_temporary(
                self.iterator,
                resolver,
                locals,
                "iterator conformance result",
            )?,
            self.interface_type.resolve(resolver).map_err(|error| {
                DefaultForIterationPlanResolutionError::Type {
                    context: "iterator conformance interface",
                    error,
                }
            })?,
            self.definition_origin
                .resolve(resolver)
                .map_err(DefaultForIterationPlanResolutionError::ConformanceDefinitionOrigin)?,
        ))
    }
}

impl DecodedDefaultIteratorNextV1 {
    fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultIteratorNextV1, DefaultForIterationPlanResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        Ok(DefaultIteratorNextV1::new(
            self.callable
                .resolve(resolver)
                .map_err(DefaultForIterationPlanResolutionError::NextCallable)?,
            resolve_temporary(self.result, resolver, locals, "iterator next result")?,
            resolve_applied_option(self.option, resolver)?,
            resolve_temporary(self.element, resolver, locals, "iterator next element")?,
            self.definition_origin
                .resolve(resolver)
                .map_err(DefaultForIterationPlanResolutionError::NextDefinitionOrigin)?,
        ))
    }
}

impl DecodedDefaultForIterationPlanV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultForIterationPlanV1, DefaultForIterationPlanResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        require_for_statement_count(
            self.source_setup.len(),
            DefaultForIterationPlanBuildError::TooManySourceSetup,
        )?;
        require_for_statement_count(
            self.iterator_setup.len(),
            DefaultForIterationPlanBuildError::TooManyIteratorSetup,
        )?;
        require_for_statement_count(
            self.body.len(),
            DefaultForIterationPlanBuildError::TooManyBodyStatements,
        )?;
        let source_setup = resolve_statements(
            self.source_setup,
            resolver,
            locals,
            StatementContext::SourceSetup,
        )?;
        let source = resolve_temporary(self.source, resolver, locals, "for source")?;
        let source_init = self
            .source_init
            .resolve(resolver, locals)
            .map_err(|error| DefaultForIterationPlanResolutionError::Expression {
                context: "for source init",
                error,
            })?;
        let iterator_setup = resolve_statements(
            self.iterator_setup,
            resolver,
            locals,
            StatementContext::IteratorSetup,
        )?;
        let iterator_call = self
            .iterator_call
            .resolve(resolver, locals)
            .map_err(|error| DefaultForIterationPlanResolutionError::Expression {
                context: "for iterator call",
                error,
            })?;
        let conformance = self.conformance.resolve(resolver, locals)?;
        let next = self.next.resolve(resolver, locals)?;
        let binding = self.binding.resolve(resolver, locals)?;
        let body = resolve_statements(self.body, resolver, locals, StatementContext::Body)?;
        DefaultForIterationPlanV1::try_new(
            source_setup,
            source,
            source_init,
            iterator_setup,
            iterator_call,
            conformance,
            next,
            binding,
            body,
        )
        .map_err(DefaultForIterationPlanResolutionError::ForShape)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultForIterationPlanResolutionError<E, L> {
    Temporary {
        context: &'static str,
        error: DefaultBindingTemporaryResolutionError<E, L>,
    },
    BindingLeaf {
        action_index: usize,
        error: DefaultBindingLeafResolutionError<E, L>,
    },
    BindingShape(DefaultBindingShapeResolutionError<E, L>),
    Projection {
        action_index: usize,
        error: DefaultBindingProjectionResolutionError<E>,
    },
    Expression {
        context: &'static str,
        error: DefaultExpressionResolutionError<E, L>,
    },
    NextCallable(DefaultCallableRefResolutionError<E>),
    OptionPayload(DefaultEnumVariantFieldRefResolutionError<E>),
    OptionNone(DefaultEnumVariantRefResolutionError<E>),
    Type {
        context: &'static str,
        error: E,
    },
    ActionDefinitionOrigin {
        action_index: usize,
        error: SourceOriginResolutionError<E>,
    },
    ConformanceDefinitionOrigin(SourceOriginResolutionError<E>),
    NextDefinitionOrigin(SourceOriginResolutionError<E>),
    Statement {
        context: StatementContext,
        index: usize,
        error: Box<DefaultStatementResolutionError<E, L>>,
    },
    ActionShape(DefaultBindingActionBuildError),
    BindingPlanShape(DefaultBindingPlanBuildError),
    ForShape(DefaultForIterationPlanBuildError),
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

impl<E: fmt::Display, L: fmt::Display> fmt::Display
    for DefaultForIterationPlanResolutionError<E, L>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Temporary { context, error } => {
                write!(formatter, "invalid default {context}: {error}")
            }
            Self::BindingLeaf {
                action_index,
                error,
            } => write!(
                formatter,
                "invalid default binding action {action_index} target: {error}"
            ),
            Self::BindingShape(error) => write!(formatter, "invalid binding shape: {error}"),
            Self::Projection {
                action_index,
                error,
            } => write!(
                formatter,
                "invalid default binding action {action_index} projection: {error}"
            ),
            Self::Expression { context, error } => {
                write!(formatter, "invalid default {context}: {error}")
            }
            Self::NextCallable(error) => {
                write!(formatter, "invalid default iterator next callable: {error}")
            }
            Self::OptionPayload(error) => {
                write!(formatter, "invalid default applied Option payload: {error}")
            }
            Self::OptionNone(error) => {
                write!(formatter, "invalid default applied Option None: {error}")
            }
            Self::Type { context, error } => {
                write!(formatter, "invalid default {context} type: {error}")
            }
            Self::ActionDefinitionOrigin {
                action_index,
                error,
            } => write!(
                formatter,
                "invalid default binding action {action_index} definition origin: {error}"
            ),
            Self::ConformanceDefinitionOrigin(error) => write!(
                formatter,
                "invalid default iterator conformance definition origin: {error}"
            ),
            Self::NextDefinitionOrigin(error) => write!(
                formatter,
                "invalid default iterator next definition origin: {error}"
            ),
            Self::Statement {
                context,
                index,
                error,
            } => write!(
                formatter,
                "invalid default for {context} statement {index}: {error}"
            ),
            Self::ActionShape(error) => error.fmt(formatter),
            Self::BindingPlanShape(error) => error.fmt(formatter),
            Self::ForShape(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static, L: std::error::Error + 'static> std::error::Error
    for DefaultForIterationPlanResolutionError<E, L>
{
}

fn resolve_temporary<R, L, E>(
    temporary: DecodedDefaultBindingTemporaryV1,
    resolver: &mut R,
    locals: &mut L,
    context: &'static str,
) -> Result<crate::DefaultBindingTemporaryV1, DefaultForIterationPlanResolutionError<E, L::Error>>
where
    R: DefaultStatementReferenceResolver<E>,
    L: TemplateLocalSelectorResolver,
{
    temporary
        .resolve(resolver, locals)
        .map_err(|error| DefaultForIterationPlanResolutionError::Temporary { context, error })
}

fn resolve_applied_option<R, L, E>(
    option: DecodedDefaultAppliedOptionV1,
    resolver: &mut R,
) -> Result<DefaultAppliedOptionV1, DefaultForIterationPlanResolutionError<E, L>>
where
    R: DefaultStatementReferenceResolver<E>,
{
    Ok(DefaultAppliedOptionV1::new(
        option
            .some_payload
            .resolve(resolver)
            .map_err(DefaultForIterationPlanResolutionError::OptionPayload)?,
        option
            .none
            .resolve(resolver)
            .map_err(DefaultForIterationPlanResolutionError::OptionNone)?,
    ))
}

fn resolve_action_origin<R, E, L>(
    origin: DecodedExportDefinitionSourceV1,
    resolver: &mut R,
    action_index: usize,
) -> Result<crate::ExportDefinitionSourceV1, DefaultForIterationPlanResolutionError<E, L>>
where
    R: DefaultStatementReferenceResolver<E>,
{
    origin.resolve(resolver).map_err(|error| {
        DefaultForIterationPlanResolutionError::ActionDefinitionOrigin {
            action_index,
            error,
        }
    })
}

fn resolve_statements<R, L, E>(
    statements: Vec<DecodedDefaultStatementV1>,
    resolver: &mut R,
    locals: &mut L,
    context: StatementContext,
) -> Result<Vec<DefaultStatementV1>, DefaultForIterationPlanResolutionError<E, L::Error>>
where
    R: DefaultStatementReferenceResolver<E>,
    L: TemplateLocalSelectorResolver,
{
    let mut resolved = Vec::with_capacity(statements.len());
    for (index, statement) in statements.into_iter().enumerate() {
        resolved.push(statement.resolve(resolver, locals).map_err(|error| {
            DefaultForIterationPlanResolutionError::Statement {
                context,
                index,
                error: Box::new(error),
            }
        })?);
    }
    Ok(resolved)
}

fn require_action_setup<E, L>(
    length: usize,
) -> Result<(), DefaultForIterationPlanResolutionError<E, L>> {
    u32::try_from(length).map(|_| ()).map_err(|_| {
        DefaultForIterationPlanResolutionError::ActionShape(
            DefaultBindingActionBuildError::TooManySetup,
        )
    })
}

fn require_action_count<E, L>(
    length: usize,
) -> Result<(), DefaultForIterationPlanResolutionError<E, L>> {
    u32::try_from(length).map(|_| ()).map_err(|_| {
        DefaultForIterationPlanResolutionError::BindingPlanShape(
            DefaultBindingPlanBuildError::TooManyActions,
        )
    })
}

fn require_for_statement_count<E, L>(
    length: usize,
    error: DefaultForIterationPlanBuildError,
) -> Result<(), DefaultForIterationPlanResolutionError<E, L>> {
    u32::try_from(length)
        .map(|_| ())
        .map_err(|_| DefaultForIterationPlanResolutionError::ForShape(error))
}

mod wire;
