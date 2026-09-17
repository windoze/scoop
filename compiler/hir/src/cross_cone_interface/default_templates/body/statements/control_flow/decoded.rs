use std::fmt;

use scoop_identity::{DecodedSignatureTypeKey, SourceOriginResolutionError};

use super::{
    DefaultCatchV1, DefaultControlFlowBuildError, DefaultTryV1, DefaultWhenArmV1,
    DefaultWhenFallbackV1, DefaultWhenGuardV1, DefaultWhenV1, OptionalDefaultStatementListV1,
    OptionalDefaultWhenGuardV1,
};
use crate::{
    DecodedDefaultExpressionV1, DecodedDefaultPatternV1, DecodedExportDefinitionSourceV1,
    DefaultExpressionResolutionError, DefaultPatternResolutionError, TemplateLocalSelectorResolver,
};

use super::super::{
    DecodedDefaultStatementV1, DefaultStatementReferenceResolver, DefaultStatementResolutionError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultWhenV1 {
    subject: DecodedDefaultExpressionV1,
    arms: Vec<DecodedDefaultWhenArmV1>,
    fallback: DecodedDefaultWhenFallbackV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultWhenArmV1 {
    pattern: DecodedDefaultPatternV1,
    guard: DecodedOptionalDefaultWhenGuardV1,
    body: Vec<DecodedDefaultStatementV1>,
    definition_origin: DecodedExportDefinitionSourceV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedOptionalDefaultWhenGuardV1 {
    Absent,
    Present(Box<DecodedDefaultWhenGuardV1>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultWhenGuardV1 {
    setup: Vec<DecodedDefaultStatementV1>,
    condition: DecodedDefaultExpressionV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDefaultWhenFallbackV1 {
    Else(Vec<DecodedDefaultStatementV1>),
    IrrefutableArm {
        subject_type: DecodedSignatureTypeKey,
    },
    PatternMatrix {
        subject_type: DecodedSignatureTypeKey,
    },
    EnumPatternMatrix {
        subject_type: DecodedSignatureTypeKey,
        owner_type: DecodedSignatureTypeKey,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultTryV1 {
    body: Vec<DecodedDefaultStatementV1>,
    catches: Vec<DecodedDefaultCatchV1>,
    finally_body: DecodedOptionalDefaultStatementListV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultCatchV1 {
    local_index: u32,
    value_type: DecodedSignatureTypeKey,
    body: Vec<DecodedDefaultStatementV1>,
    definition_origin: DecodedExportDefinitionSourceV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedOptionalDefaultStatementListV1 {
    Absent,
    Present(Vec<DecodedDefaultStatementV1>),
}

impl DecodedDefaultWhenV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultWhenV1, DefaultControlFlowResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        require_count(
            self.arms.len(),
            DefaultControlFlowBuildError::TooManyWhenArms,
        )?;
        let subject = self.subject.resolve(resolver, locals).map_err(|error| {
            DefaultControlFlowResolutionError::Expression {
                context: "when subject",
                error,
            }
        })?;
        let mut arms = Vec::with_capacity(self.arms.len());
        for (index, arm) in self.arms.into_iter().enumerate() {
            arms.push(arm.resolve(resolver, locals, index)?);
        }
        DefaultWhenV1::try_new(subject, arms, self.fallback.resolve(resolver, locals)?)
            .map_err(DefaultControlFlowResolutionError::Shape)
    }
}

impl DecodedDefaultWhenArmV1 {
    fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
        arm_index: usize,
    ) -> Result<DefaultWhenArmV1, DefaultControlFlowResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        require_statement_count(self.body.len())?;
        let pattern = self
            .pattern
            .resolve(resolver, locals)
            .map_err(|error| DefaultControlFlowResolutionError::Pattern { arm_index, error })?;
        let guard = self.guard.resolve(resolver, locals, arm_index)?;
        let body = resolve_statements(
            self.body,
            resolver,
            locals,
            StatementContext::WhenArm { arm_index },
        )?;
        let definition_origin = self.definition_origin.resolve(resolver).map_err(|error| {
            DefaultControlFlowResolutionError::DefinitionOrigin {
                context: OriginContext::WhenArm { arm_index },
                error,
            }
        })?;
        DefaultWhenArmV1::try_new(pattern, guard, body, definition_origin)
            .map_err(DefaultControlFlowResolutionError::Shape)
    }
}

impl DecodedOptionalDefaultWhenGuardV1 {
    fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
        arm_index: usize,
    ) -> Result<OptionalDefaultWhenGuardV1, DefaultControlFlowResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        match self {
            Self::Absent => Ok(OptionalDefaultWhenGuardV1::absent()),
            Self::Present(guard) => (*guard)
                .resolve(resolver, locals, arm_index)
                .map(OptionalDefaultWhenGuardV1::present),
        }
    }
}

impl DecodedDefaultWhenGuardV1 {
    fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
        arm_index: usize,
    ) -> Result<DefaultWhenGuardV1, DefaultControlFlowResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        require_statement_count(self.setup.len())?;
        let setup = resolve_statements(
            self.setup,
            resolver,
            locals,
            StatementContext::WhenGuard { arm_index },
        )?;
        let condition = self.condition.resolve(resolver, locals).map_err(|error| {
            DefaultControlFlowResolutionError::Expression {
                context: "when guard condition",
                error,
            }
        })?;
        DefaultWhenGuardV1::try_new(setup, condition)
            .map_err(DefaultControlFlowResolutionError::Shape)
    }
}

impl DecodedDefaultWhenFallbackV1 {
    fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultWhenFallbackV1, DefaultControlFlowResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        match self {
            Self::Else(statements) => {
                require_statement_count(statements.len())?;
                let statements =
                    resolve_statements(statements, resolver, locals, StatementContext::WhenElse)?;
                DefaultWhenFallbackV1::try_else(statements)
                    .map_err(DefaultControlFlowResolutionError::Shape)
            }
            Self::IrrefutableArm { subject_type } => subject_type
                .resolve(resolver)
                .map(DefaultWhenFallbackV1::irrefutable_arm)
                .map_err(|error| DefaultControlFlowResolutionError::Type {
                    context: "when irrefutable subject",
                    error,
                }),
            Self::PatternMatrix { subject_type } => subject_type
                .resolve(resolver)
                .map(DefaultWhenFallbackV1::pattern_matrix)
                .map_err(|error| DefaultControlFlowResolutionError::Type {
                    context: "when pattern-matrix subject",
                    error,
                }),
            Self::EnumPatternMatrix {
                subject_type,
                owner_type,
            } => Ok(DefaultWhenFallbackV1::enum_pattern_matrix(
                subject_type.resolve(resolver).map_err(|error| {
                    DefaultControlFlowResolutionError::Type {
                        context: "when enum-pattern subject",
                        error,
                    }
                })?,
                owner_type.resolve(resolver).map_err(|error| {
                    DefaultControlFlowResolutionError::Type {
                        context: "when enum-pattern owner",
                        error,
                    }
                })?,
            )),
        }
    }
}

impl DecodedDefaultTryV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultTryV1, DefaultControlFlowResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        require_statement_count(self.body.len())?;
        require_count(
            self.catches.len(),
            DefaultControlFlowBuildError::TooManyCatches,
        )?;
        let body = resolve_statements(self.body, resolver, locals, StatementContext::TryBody)?;
        let mut catches = Vec::with_capacity(self.catches.len());
        for (index, catch) in self.catches.into_iter().enumerate() {
            catches.push(catch.resolve(resolver, locals, index)?);
        }
        let finally_body = self.finally_body.resolve(resolver, locals)?;
        DefaultTryV1::try_new(body, catches, finally_body)
            .map_err(DefaultControlFlowResolutionError::Shape)
    }
}

impl DecodedDefaultCatchV1 {
    fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
        catch_index: usize,
    ) -> Result<DefaultCatchV1, DefaultControlFlowResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        require_statement_count(self.body.len())?;
        let local = locals
            .resolve_template_local_selector(self.local_index)
            .map_err(|error| DefaultControlFlowResolutionError::CatchLocal {
                catch_index,
                error,
            })?;
        let value_type = self
            .value_type
            .resolve(resolver)
            .map_err(|error| DefaultControlFlowResolutionError::CatchType { catch_index, error })?;
        let body = resolve_statements(
            self.body,
            resolver,
            locals,
            StatementContext::Catch { catch_index },
        )?;
        let definition_origin = self.definition_origin.resolve(resolver).map_err(|error| {
            DefaultControlFlowResolutionError::DefinitionOrigin {
                context: OriginContext::Catch { catch_index },
                error,
            }
        })?;
        DefaultCatchV1::try_new(local, value_type, body, definition_origin)
            .map_err(DefaultControlFlowResolutionError::Shape)
    }
}

impl DecodedOptionalDefaultStatementListV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<OptionalDefaultStatementListV1, DefaultControlFlowResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        match self {
            Self::Absent => Ok(OptionalDefaultStatementListV1::absent()),
            Self::Present(statements) => {
                require_statement_count(statements.len())?;
                let statements =
                    resolve_statements(statements, resolver, locals, StatementContext::Optional)?;
                OptionalDefaultStatementListV1::try_present(statements)
                    .map_err(DefaultControlFlowResolutionError::Shape)
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultControlFlowResolutionError<E, L> {
    Expression {
        context: &'static str,
        error: DefaultExpressionResolutionError<E, L>,
    },
    Pattern {
        arm_index: usize,
        error: DefaultPatternResolutionError<E, L>,
    },
    Type {
        context: &'static str,
        error: E,
    },
    CatchLocal {
        catch_index: usize,
        error: L,
    },
    CatchType {
        catch_index: usize,
        error: E,
    },
    DefinitionOrigin {
        context: OriginContext,
        error: SourceOriginResolutionError<E>,
    },
    Statement {
        context: StatementContext,
        index: usize,
        error: Box<DefaultStatementResolutionError<E, L>>,
    },
    Shape(DefaultControlFlowBuildError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatementContext {
    WhenArm { arm_index: usize },
    WhenGuard { arm_index: usize },
    WhenElse,
    TryBody,
    Catch { catch_index: usize },
    Optional,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OriginContext {
    WhenArm { arm_index: usize },
    Catch { catch_index: usize },
}

impl fmt::Display for StatementContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WhenArm { arm_index } => write!(formatter, "when arm {arm_index} body"),
            Self::WhenGuard { arm_index } => write!(formatter, "when arm {arm_index} guard setup"),
            Self::WhenElse => formatter.write_str("when else"),
            Self::TryBody => formatter.write_str("try body"),
            Self::Catch { catch_index } => write!(formatter, "catch {catch_index} body"),
            Self::Optional => formatter.write_str("optional body"),
        }
    }
}

impl fmt::Display for OriginContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WhenArm { arm_index } => write!(formatter, "when arm {arm_index}"),
            Self::Catch { catch_index } => write!(formatter, "catch {catch_index}"),
        }
    }
}

impl<E: fmt::Display, L: fmt::Display> fmt::Display for DefaultControlFlowResolutionError<E, L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expression { context, error } => {
                write!(formatter, "invalid default {context}: {error}")
            }
            Self::Pattern { arm_index, error } => {
                write!(
                    formatter,
                    "invalid default when arm {arm_index} pattern: {error}"
                )
            }
            Self::Type { context, error } => {
                write!(formatter, "invalid default {context} type: {error}")
            }
            Self::CatchLocal { catch_index, error } => {
                write!(
                    formatter,
                    "invalid default catch {catch_index} local: {error}"
                )
            }
            Self::CatchType { catch_index, error } => {
                write!(
                    formatter,
                    "invalid default catch {catch_index} type: {error}"
                )
            }
            Self::DefinitionOrigin { context, error } => {
                write!(
                    formatter,
                    "invalid default {context} definition origin: {error}"
                )
            }
            Self::Statement {
                context,
                index,
                error,
            } => write!(
                formatter,
                "invalid default {context} statement {index}: {error}"
            ),
            Self::Shape(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static, L: std::error::Error + 'static> std::error::Error
    for DefaultControlFlowResolutionError<E, L>
{
}

fn require_count<E, L>(
    length: usize,
    error: DefaultControlFlowBuildError,
) -> Result<(), DefaultControlFlowResolutionError<E, L>> {
    u32::try_from(length)
        .map(|_| ())
        .map_err(|_| DefaultControlFlowResolutionError::Shape(error))
}

fn require_statement_count<E, L>(
    length: usize,
) -> Result<(), DefaultControlFlowResolutionError<E, L>> {
    require_count(length, DefaultControlFlowBuildError::TooManyStatements)
}

fn resolve_statements<R, L, E>(
    statements: Vec<DecodedDefaultStatementV1>,
    resolver: &mut R,
    locals: &mut L,
    context: StatementContext,
) -> Result<Vec<super::super::DefaultStatementV1>, DefaultControlFlowResolutionError<E, L::Error>>
where
    R: DefaultStatementReferenceResolver<E>,
    L: TemplateLocalSelectorResolver,
{
    let mut resolved = Vec::with_capacity(statements.len());
    for (index, statement) in statements.into_iter().enumerate() {
        resolved.push(statement.resolve(resolver, locals).map_err(|error| {
            DefaultControlFlowResolutionError::Statement {
                context,
                index,
                error: Box::new(error),
            }
        })?);
    }
    Ok(resolved)
}

mod wire;
