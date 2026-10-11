use std::fmt;

use scoop_identity::{LocalValueSelector, SignatureTypeKey};

use super::DefaultStatementV1;
use crate::{DefaultExpressionV1, DefaultPatternV1, ExportDefinitionSourceV1};

mod decoded;
mod indexed;

pub use decoded::{
    DecodedDefaultCatchV1, DecodedDefaultTryV1, DecodedDefaultWhenArmV1,
    DecodedDefaultWhenConditionV1, DecodedDefaultWhenFallbackV1, DecodedDefaultWhenGuardV1,
    DecodedDefaultWhenV1, DecodedOptionalDefaultStatementListV1, DecodedOptionalDefaultWhenGuardV1,
    DefaultControlFlowResolutionError,
};

pub use indexed::{
    DefaultControlFlowIndexError, IndexedDefaultCatchV1, IndexedDefaultTryV1,
    IndexedDefaultWhenArmV1, IndexedDefaultWhenConditionV1, IndexedDefaultWhenFallbackV1,
    IndexedDefaultWhenGuardV1, IndexedDefaultWhenV1, IndexedOptionalDefaultStatementListV1,
    IndexedOptionalDefaultWhenGuardV1,
};

mod when;
pub use when::*;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultTryV1 {
    body: Vec<DefaultStatementV1>,
    catches: Vec<DefaultCatchV1>,
    finally_body: OptionalDefaultStatementListV1,
}

impl DefaultTryV1 {
    pub fn try_new(
        body: Vec<DefaultStatementV1>,
        catches: Vec<DefaultCatchV1>,
        finally_body: OptionalDefaultStatementListV1,
    ) -> Result<Self, DefaultControlFlowBuildError> {
        require_statements(&body)?;
        require_len(catches.len(), DefaultControlFlowBuildError::TooManyCatches)?;
        Ok(Self {
            body,
            catches,
            finally_body,
        })
    }

    pub fn body(&self) -> &[DefaultStatementV1] {
        &self.body
    }

    pub fn catches(&self) -> &[DefaultCatchV1] {
        &self.catches
    }

    pub const fn finally_body(&self) -> &OptionalDefaultStatementListV1 {
        &self.finally_body
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultCatchV1 {
    local: LocalValueSelector,
    value_type: SignatureTypeKey,
    body: Vec<DefaultStatementV1>,
    definition_origin: ExportDefinitionSourceV1,
}

impl DefaultCatchV1 {
    pub fn try_new(
        local: LocalValueSelector,
        value_type: SignatureTypeKey,
        body: Vec<DefaultStatementV1>,
        definition_origin: ExportDefinitionSourceV1,
    ) -> Result<Self, DefaultControlFlowBuildError> {
        require_statements(&body)?;
        Ok(Self {
            local,
            value_type,
            body,
            definition_origin,
        })
    }

    pub const fn local(&self) -> &LocalValueSelector {
        &self.local
    }

    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }

    pub fn body(&self) -> &[DefaultStatementV1] {
        &self.body
    }

    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OptionalDefaultStatementListV1(OptionalDefaultStatementListKindV1);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum OptionalDefaultStatementListKindV1 {
    Absent,
    Present(Vec<DefaultStatementV1>),
}

#[derive(Clone, Copy, Debug)]
pub enum OptionalDefaultStatementListViewV1<'a> {
    Absent,
    Present(&'a [DefaultStatementV1]),
}

impl OptionalDefaultStatementListV1 {
    pub const fn absent() -> Self {
        Self(OptionalDefaultStatementListKindV1::Absent)
    }

    pub fn try_present(
        statements: Vec<DefaultStatementV1>,
    ) -> Result<Self, DefaultControlFlowBuildError> {
        require_statements(&statements)?;
        Ok(Self(OptionalDefaultStatementListKindV1::Present(
            statements,
        )))
    }

    pub fn view(&self) -> OptionalDefaultStatementListViewV1<'_> {
        match &self.0 {
            OptionalDefaultStatementListKindV1::Absent => {
                OptionalDefaultStatementListViewV1::Absent
            }
            OptionalDefaultStatementListKindV1::Present(statements) => {
                OptionalDefaultStatementListViewV1::Present(statements)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultControlFlowBuildError {
    MissingWhenSubject,
    WhenSubjectTypeMismatch,
    TooManyStatements,
    TooManyWhenArms,
    TooManyCatches,
}

impl fmt::Display for DefaultControlFlowBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WhenSubjectTypeMismatch => {
                formatter.write_str("when fallback type does not match its subject")
            }
            Self::TooManyStatements => {
                formatter.write_str("default control-flow statement count exceeds u32")
            }
            Self::MissingWhenSubject => {
                formatter.write_str("a case or exhaustiveness proof requires a when subject")
            }
            Self::TooManyWhenArms => formatter.write_str("default when arm count exceeds u32"),
            Self::TooManyCatches => formatter.write_str("default catch count exceeds u32"),
        }
    }
}

impl std::error::Error for DefaultControlFlowBuildError {}

pub(super) fn require_statements(
    statements: &[DefaultStatementV1],
) -> Result<(), DefaultControlFlowBuildError> {
    require_len(
        statements.len(),
        DefaultControlFlowBuildError::TooManyStatements,
    )
    .map(|_| ())
}

fn require_len<E>(length: usize, error: E) -> Result<u32, E> {
    u32::try_from(length).map_err(|_| error)
}
