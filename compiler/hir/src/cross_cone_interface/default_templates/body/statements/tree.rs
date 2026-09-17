use std::fmt;

use scoop_identity::{PersistentIdResolver, PersistentInitializationUnitId};

use super::{
    DefaultAssignTargetV1, DefaultForIterationPlanV1, DefaultTryV1, DefaultWhenV1,
    OptionalDefaultStatementListV1, OptionalDefaultStatementListViewV1,
};
use crate::{
    DefaultExpressionReferenceResolver, DefaultExpressionV1, DefaultLocalFunctionV1,
    DefaultPatternV1, ExportDefinitionSourceV1,
};

mod decoded;
mod indexed;

pub use decoded::{DecodedDefaultStatementV1, DefaultStatementResolutionError};

pub use indexed::{DefaultStatementIndexError, IndexedDefaultStatementV1};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultStatementV1 {
    kind: DefaultStatementKindV1,
    definition_origin: ExportDefinitionSourceV1,
}

impl DefaultStatementV1 {
    pub fn try_new(
        kind: DefaultStatementKindV1,
        definition_origin: ExportDefinitionSourceV1,
    ) -> Result<Self, DefaultStatementBuildError> {
        validate_kind(&kind)?;
        Ok(Self {
            kind,
            definition_origin,
        })
    }

    pub const fn kind(&self) -> &DefaultStatementKindV1 {
        &self.kind
    }

    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultStatementKindV1 {
    Expr(Box<DefaultExpressionV1>),
    InitializationEnsure(PersistentInitializationUnitId),
    LocalFunction(DefaultLocalFunctionV1),
    Return(crate::OptionalDefaultExpressionV1),
    ValDecl {
        pattern: DefaultPatternV1,
        init: Box<DefaultExpressionV1>,
    },
    Assign {
        target: Box<DefaultAssignTargetV1>,
        value: Box<DefaultExpressionV1>,
    },
    If {
        condition: Box<DefaultExpressionV1>,
        then_body: Vec<DefaultStatementV1>,
        else_body: OptionalDefaultStatementListV1,
    },
    While {
        condition_setup: Vec<DefaultStatementV1>,
        condition: Box<DefaultExpressionV1>,
        body: Vec<DefaultStatementV1>,
    },
    For(Box<DefaultForIterationPlanV1>),
    Break,
    Continue,
    When(Box<DefaultWhenV1>),
    Try(Box<DefaultTryV1>),
    Throw(Box<DefaultExpressionV1>),
}

pub trait DefaultStatementReferenceResolver<E>:
    DefaultExpressionReferenceResolver<E>
    + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
{
}

impl<R, E> DefaultStatementReferenceResolver<E> for R where
    R: DefaultExpressionReferenceResolver<E>
        + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
{
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultStatementBuildError {
    TooManyStatements { variant_tag: u64, field: u32 },
}

impl fmt::Display for DefaultStatementBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyStatements { variant_tag, field } => write!(
                formatter,
                "default statement tag {variant_tag} field {field} count exceeds u32"
            ),
        }
    }
}

impl std::error::Error for DefaultStatementBuildError {}

fn validate_kind(kind: &DefaultStatementKindV1) -> Result<(), DefaultStatementBuildError> {
    match kind {
        DefaultStatementKindV1::If {
            then_body,
            else_body,
            ..
        } => {
            require_statements(then_body, 7, 2)?;
            if let OptionalDefaultStatementListViewV1::Present(statements) = else_body.view() {
                require_statements(statements, 7, 3)?;
            }
        }
        DefaultStatementKindV1::While {
            condition_setup,
            body,
            ..
        } => {
            require_statements(condition_setup, 8, 1)?;
            require_statements(body, 8, 3)?;
        }
        DefaultStatementKindV1::Expr(_)
        | DefaultStatementKindV1::InitializationEnsure(_)
        | DefaultStatementKindV1::LocalFunction(_)
        | DefaultStatementKindV1::Return(_)
        | DefaultStatementKindV1::ValDecl { .. }
        | DefaultStatementKindV1::Assign { .. }
        | DefaultStatementKindV1::For(_)
        | DefaultStatementKindV1::Break
        | DefaultStatementKindV1::Continue
        | DefaultStatementKindV1::When(_)
        | DefaultStatementKindV1::Try(_)
        | DefaultStatementKindV1::Throw(_) => {}
    }
    Ok(())
}

fn require_statements(
    statements: &[DefaultStatementV1],
    variant_tag: u64,
    field: u32,
) -> Result<(), DefaultStatementBuildError> {
    u32::try_from(statements.len())
        .map(|_| ())
        .map_err(|_| DefaultStatementBuildError::TooManyStatements { variant_tag, field })
}

#[cfg(test)]
mod tests;
