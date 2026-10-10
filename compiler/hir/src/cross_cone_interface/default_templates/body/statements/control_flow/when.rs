use super::*;
use crate::ExhaustivenessProof;
use std::convert::Infallible;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultWhenV1 {
    Subject {
        subject: Box<DefaultExpressionV1>,
        arms: Vec<DefaultWhenArmV1>,
        fallback: DefaultWhenFallbackV1,
    },
    Conditions {
        arms: Vec<DefaultWhenArmV1<Infallible>>,
        fallback: DefaultWhenFallbackV1<Infallible>,
    },
}

impl DefaultWhenV1 {
    pub fn try_subject(
        subject: DefaultExpressionV1,
        arms: Vec<DefaultWhenArmV1>,
        fallback: DefaultWhenFallbackV1,
    ) -> Result<Self, DefaultControlFlowBuildError> {
        require_len(arms.len(), DefaultControlFlowBuildError::TooManyWhenArms)?;
        Ok(Self::Subject {
            subject: Box::new(subject),
            arms,
            fallback,
        })
    }

    pub fn try_conditions(
        arms: Vec<DefaultWhenArmV1<Infallible>>,
        fallback: DefaultWhenFallbackV1<Infallible>,
    ) -> Result<Self, DefaultControlFlowBuildError> {
        require_len(arms.len(), DefaultControlFlowBuildError::TooManyWhenArms)?;
        Ok(Self::Conditions { arms, fallback })
    }

    pub fn subject(&self) -> Option<&DefaultExpressionV1> {
        match self {
            Self::Subject { subject, .. } => Some(subject),
            Self::Conditions { .. } => None,
        }
    }

    pub fn arm_count(&self) -> usize {
        match self {
            Self::Subject { arms, .. } => arms.len(),
            Self::Conditions { arms, .. } => arms.len(),
        }
    }

    pub fn arms(&self) -> impl DoubleEndedIterator<Item = DefaultWhenArmRefV1<'_>> {
        let (subject, conditions): (&[DefaultWhenArmV1], &[DefaultWhenArmV1<Infallible>]) =
            match self {
                Self::Subject { arms, .. } => (arms, &[]),
                Self::Conditions { arms, .. } => (&[], arms),
            };
        subject
            .iter()
            .map(|arm| arm_view(arm, |pattern| pattern))
            .chain(
                conditions
                    .iter()
                    .map(|arm| arm_view(arm, |never| match *never {})),
            )
    }

    /// Wire/reference views obtain every coverage type from the same subject.
    pub fn fallback(&self) -> DefaultWhenFallbackViewV1<'_> {
        match self {
            Self::Subject {
                subject, fallback, ..
            } => match fallback {
                DefaultWhenFallbackV1::Fallthrough => DefaultWhenFallbackViewV1::Fallthrough,
                DefaultWhenFallbackV1::Else(body) => DefaultWhenFallbackViewV1::Else(body),
                DefaultWhenFallbackV1::Impossible(proof) => {
                    let subject_type = subject.result_type();
                    match proof {
                        ExhaustivenessProof::IrrefutableArm => {
                            DefaultWhenFallbackViewV1::IrrefutableArm { subject_type }
                        }
                        ExhaustivenessProof::PatternMatrix => {
                            DefaultWhenFallbackViewV1::PatternMatrix { subject_type }
                        }
                        ExhaustivenessProof::EnumPatternMatrix => {
                            DefaultWhenFallbackViewV1::EnumPatternMatrix {
                                subject_type,
                                owner_type: subject_type,
                            }
                        }
                    }
                }
            },
            Self::Conditions { fallback, .. } => match fallback {
                DefaultWhenFallbackV1::Fallthrough => DefaultWhenFallbackViewV1::Fallthrough,
                DefaultWhenFallbackV1::Else(body) => DefaultWhenFallbackViewV1::Else(body),
                DefaultWhenFallbackV1::Impossible(never) => match *never {},
            },
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultWhenFallbackV1<P = ExhaustivenessProof> {
    Fallthrough,
    Else(Vec<DefaultStatementV1>),
    Impossible(P),
}

impl<P> DefaultWhenFallbackV1<P> {
    pub const fn fallthrough() -> Self {
        Self::Fallthrough
    }
    pub fn try_else(
        statements: Vec<DefaultStatementV1>,
    ) -> Result<Self, DefaultControlFlowBuildError> {
        require_statements(&statements)?;
        Ok(Self::Else(statements))
    }
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

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultWhenArmV1<P = DefaultPatternV1> {
    pub(super) condition: DefaultWhenConditionV1<P>,
    pub(super) guard: OptionalDefaultWhenGuardV1,
    pub(super) body: Vec<DefaultStatementV1>,
    pub(super) definition_origin: ExportDefinitionSourceV1,
}

impl<P> DefaultWhenArmV1<P> {
    pub fn try_new(
        condition: DefaultWhenConditionV1<P>,
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

    pub const fn condition(&self) -> &DefaultWhenConditionV1<P> {
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultWhenConditionV1<P = DefaultPatternV1, G = Box<DefaultWhenGuardV1>> {
    Case(P),
    Predicate(G),
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
    pub(super) setup: Vec<DefaultStatementV1>,
    pub(super) condition: DefaultExpressionV1,
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

#[derive(Clone, Copy, Debug)]
pub struct DefaultWhenArmRefV1<'a> {
    condition: DefaultWhenConditionV1<&'a DefaultPatternV1, &'a DefaultWhenGuardV1>,
    guard: &'a OptionalDefaultWhenGuardV1,
    body: &'a [DefaultStatementV1],
    definition_origin: &'a ExportDefinitionSourceV1,
}

impl<'a> DefaultWhenArmRefV1<'a> {
    pub const fn condition(
        &self,
    ) -> DefaultWhenConditionV1<&'a DefaultPatternV1, &'a DefaultWhenGuardV1> {
        self.condition
    }
    pub const fn guard(&self) -> &'a OptionalDefaultWhenGuardV1 {
        self.guard
    }
    pub const fn body(&self) -> &'a [DefaultStatementV1] {
        self.body
    }
    pub const fn definition_origin(&self) -> &'a ExportDefinitionSourceV1 {
        self.definition_origin
    }
}

fn arm_view<'a, P>(
    arm: &'a DefaultWhenArmV1<P>,
    pattern: impl FnOnce(&'a P) -> &'a DefaultPatternV1,
) -> DefaultWhenArmRefV1<'a> {
    DefaultWhenArmRefV1 {
        condition: match &arm.condition {
            DefaultWhenConditionV1::Case(value) => DefaultWhenConditionV1::Case(pattern(value)),
            DefaultWhenConditionV1::Predicate(predicate) => {
                DefaultWhenConditionV1::Predicate(predicate)
            }
            DefaultWhenConditionV1::Always => DefaultWhenConditionV1::Always,
        },
        guard: &arm.guard,
        body: &arm.body,
        definition_origin: &arm.definition_origin,
    }
}
