use std::fmt;

use scoop_identity::{LocalValueSelector, SignatureTypeKey};

use super::DefaultStatementV1;
use crate::{
    DefaultExpressionV1, DefaultPatternV1, ExportDefinitionSourceV1, OptionalDefaultExpressionV1,
};

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

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultWhenV1 {
    subject: OptionalDefaultExpressionV1,
    arms: Vec<DefaultWhenArmV1>,
    fallback: DefaultWhenFallbackV1,
}

impl DefaultWhenV1 {
    pub fn try_new(
        subject: OptionalDefaultExpressionV1,
        arms: Vec<DefaultWhenArmV1>,
        fallback: DefaultWhenFallbackV1,
    ) -> Result<Self, DefaultControlFlowBuildError> {
        require_len(arms.len(), DefaultControlFlowBuildError::TooManyWhenArms)?;
        if subject.as_ref().is_none()
            && (arms
                .iter()
                .any(|arm| matches!(arm.condition, DefaultWhenConditionV1::Case(_)))
                || !matches!(
                    fallback.view(),
                    DefaultWhenFallbackViewV1::Else(_) | DefaultWhenFallbackViewV1::Fallthrough
                ))
        {
            return Err(DefaultControlFlowBuildError::MissingWhenSubject);
        }
        Ok(Self {
            subject,
            arms,
            fallback,
        })
    }

    pub fn subject(&self) -> Option<&DefaultExpressionV1> {
        self.subject.as_ref()
    }

    pub fn arms(&self) -> &[DefaultWhenArmV1] {
        &self.arms
    }

    pub const fn fallback(&self) -> &DefaultWhenFallbackV1 {
        &self.fallback
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultWhenArmV1 {
    condition: DefaultWhenConditionV1,
    guard: OptionalDefaultWhenGuardV1,
    body: Vec<DefaultStatementV1>,
    definition_origin: ExportDefinitionSourceV1,
}

impl DefaultWhenArmV1 {
    pub fn try_new(
        condition: DefaultWhenConditionV1,
        guard: OptionalDefaultWhenGuardV1,
        body: Vec<DefaultStatementV1>,
        definition_origin: ExportDefinitionSourceV1,
    ) -> Result<Self, DefaultControlFlowBuildError> {
        require_statements(&body)?;
        Ok(Self {
            condition,
            guard,
            body,
            definition_origin,
        })
    }

    pub const fn condition(&self) -> &DefaultWhenConditionV1 {
        &self.condition
    }

    pub const fn guard(&self) -> &OptionalDefaultWhenGuardV1 {
        &self.guard
    }

    pub fn body(&self) -> &[DefaultStatementV1] {
        &self.body
    }

    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultWhenConditionV1 {
    Case(DefaultPatternV1),
    Predicate(Box<DefaultWhenGuardV1>),
    Always,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OptionalDefaultWhenGuardV1 {
    Absent,
    Present(Box<DefaultWhenGuardV1>),
}

impl OptionalDefaultWhenGuardV1 {
    pub const fn absent() -> Self {
        Self::Absent
    }

    pub fn present(guard: DefaultWhenGuardV1) -> Self {
        Self::Present(Box::new(guard))
    }

    pub fn as_ref(&self) -> Option<&DefaultWhenGuardV1> {
        match self {
            Self::Absent => None,
            Self::Present(guard) => Some(guard),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultWhenGuardV1 {
    setup: Vec<DefaultStatementV1>,
    condition: DefaultExpressionV1,
}

impl DefaultWhenGuardV1 {
    pub fn try_new(
        setup: Vec<DefaultStatementV1>,
        condition: DefaultExpressionV1,
    ) -> Result<Self, DefaultControlFlowBuildError> {
        require_statements(&setup)?;
        Ok(Self { setup, condition })
    }

    pub fn setup(&self) -> &[DefaultStatementV1] {
        &self.setup
    }

    pub const fn condition(&self) -> &DefaultExpressionV1 {
        &self.condition
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultWhenFallbackV1(DefaultWhenFallbackKindV1);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum DefaultWhenFallbackKindV1 {
    Fallthrough,
    Else(Vec<DefaultStatementV1>),
    IrrefutableArm {
        subject_type: SignatureTypeKey,
    },
    PatternMatrix {
        subject_type: SignatureTypeKey,
    },
    EnumPatternMatrix {
        subject_type: SignatureTypeKey,
        owner_type: SignatureTypeKey,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum DefaultWhenFallbackViewV1<'a> {
    Fallthrough,
    Else(&'a [DefaultStatementV1]),
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

impl DefaultWhenFallbackV1 {
    pub const fn fallthrough() -> Self {
        Self(DefaultWhenFallbackKindV1::Fallthrough)
    }

    pub fn try_else(
        statements: Vec<DefaultStatementV1>,
    ) -> Result<Self, DefaultControlFlowBuildError> {
        require_statements(&statements)?;
        Ok(Self(DefaultWhenFallbackKindV1::Else(statements)))
    }

    pub const fn irrefutable_arm(subject_type: SignatureTypeKey) -> Self {
        Self(DefaultWhenFallbackKindV1::IrrefutableArm { subject_type })
    }

    pub const fn pattern_matrix(subject_type: SignatureTypeKey) -> Self {
        Self(DefaultWhenFallbackKindV1::PatternMatrix { subject_type })
    }

    pub const fn enum_pattern_matrix(
        subject_type: SignatureTypeKey,
        owner_type: SignatureTypeKey,
    ) -> Self {
        Self(DefaultWhenFallbackKindV1::EnumPatternMatrix {
            subject_type,
            owner_type,
        })
    }

    pub fn view(&self) -> DefaultWhenFallbackViewV1<'_> {
        match &self.0 {
            DefaultWhenFallbackKindV1::Fallthrough => DefaultWhenFallbackViewV1::Fallthrough,
            DefaultWhenFallbackKindV1::Else(statements) => {
                DefaultWhenFallbackViewV1::Else(statements)
            }
            DefaultWhenFallbackKindV1::IrrefutableArm { subject_type } => {
                DefaultWhenFallbackViewV1::IrrefutableArm { subject_type }
            }
            DefaultWhenFallbackKindV1::PatternMatrix { subject_type } => {
                DefaultWhenFallbackViewV1::PatternMatrix { subject_type }
            }
            DefaultWhenFallbackKindV1::EnumPatternMatrix {
                subject_type,
                owner_type,
            } => DefaultWhenFallbackViewV1::EnumPatternMatrix {
                subject_type,
                owner_type,
            },
        }
    }
}

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
    TooManyStatements,
    TooManyWhenArms,
    TooManyCatches,
}

impl fmt::Display for DefaultControlFlowBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
