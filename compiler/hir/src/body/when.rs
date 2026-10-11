use super::*;
use std::convert::Infallible;

/// The presence of a subject determines the permitted conditions and fallback.
#[derive(Debug, Clone)]
pub enum When {
    Subject {
        subject: Expr,
        arms: Vec<WhenArm>,
        fallback: WhenFallback,
    },
    Conditions {
        arms: Vec<WhenArm<WhenPredicate>>,
        fallback: WhenFallback<Infallible>,
    },
}

#[derive(Debug, Clone)]
pub enum WhenFallback<P = ExhaustivenessProof> {
    Else(Vec<Statement>),
    Fallthrough,
    Impossible(P),
}

/// The exhaustiveness category established for the enclosing subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ExhaustivenessProof {
    IrrefutableArm,
    PatternMatrix,
    EnumPatternMatrix,
}

#[derive(Debug, Clone)]
pub struct WhenArm<C = WhenCondition> {
    pub condition: C,
    pub guard: Option<WhenGuard>,
    pub body: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum WhenCondition {
    Case(Pattern),
    Predicate(WhenGuard),
    Always,
}

#[derive(Debug, Clone)]
pub enum WhenPredicate {
    Predicate(WhenGuard),
    Always,
}

#[derive(Debug, Clone)]
pub struct WhenGuard {
    pub setup: Vec<Statement>,
    pub condition: Expr,
}

/// Borrowed traversal data; transformations match the complete When branch.
#[derive(Clone, Copy)]
pub struct WhenArmRef<'a> {
    pub condition: WhenConditionRef<'a>,
    pub guard: Option<&'a WhenGuard>,
    pub body: &'a [Statement],
    pub span: Span,
}

#[derive(Clone, Copy)]
pub enum WhenConditionRef<'a> {
    Case(&'a Pattern),
    Predicate(&'a WhenGuard),
    Always,
}

#[derive(Clone, Copy)]
pub enum WhenFallbackRef<'a> {
    Else(&'a [Statement]),
    Fallthrough,
    Impossible(ExhaustivenessProof),
}

impl When {
    pub fn subject(&self) -> Option<&Expr> {
        match self {
            Self::Subject { subject, .. } => Some(subject),
            Self::Conditions { .. } => None,
        }
    }

    pub fn arms(&self) -> impl DoubleEndedIterator<Item = WhenArmRef<'_>> {
        let (subject, conditions): (&[WhenArm], &[WhenArm<WhenPredicate>]) = match self {
            Self::Subject { arms, .. } => (arms, &[]),
            Self::Conditions { arms, .. } => (&[], arms),
        };
        subject
            .iter()
            .map(|arm| WhenArmRef {
                condition: match &arm.condition {
                    WhenCondition::Case(pattern) => WhenConditionRef::Case(pattern),
                    WhenCondition::Predicate(predicate) => WhenConditionRef::Predicate(predicate),
                    WhenCondition::Always => WhenConditionRef::Always,
                },
                guard: arm.guard.as_ref(),
                body: &arm.body,
                span: arm.span,
            })
            .chain(conditions.iter().map(|arm| WhenArmRef {
                condition: match &arm.condition {
                    WhenPredicate::Predicate(predicate) => WhenConditionRef::Predicate(predicate),
                    WhenPredicate::Always => WhenConditionRef::Always,
                },
                guard: arm.guard.as_ref(),
                body: &arm.body,
                span: arm.span,
            }))
    }

    pub fn fallback(&self) -> WhenFallbackRef<'_> {
        match self {
            Self::Subject { fallback, .. } => match fallback {
                WhenFallback::Else(body) => WhenFallbackRef::Else(body),
                WhenFallback::Fallthrough => WhenFallbackRef::Fallthrough,
                WhenFallback::Impossible(proof) => WhenFallbackRef::Impossible(*proof),
            },
            Self::Conditions { fallback, .. } => match fallback {
                WhenFallback::Else(body) => WhenFallbackRef::Else(body),
                WhenFallback::Fallthrough => WhenFallbackRef::Fallthrough,
                WhenFallback::Impossible(never) => match *never {},
            },
        }
    }
}

/// In-place expression/body rewriting cannot change a condition's category.
pub struct WhenArmMut<'a> {
    pub condition: WhenConditionMut<'a>,
    pub guard: Option<&'a mut WhenGuard>,
    pub body: &'a mut Vec<Statement>,
}

pub enum WhenConditionMut<'a> {
    Case(&'a mut Pattern),
    Predicate(&'a mut WhenGuard),
    Always,
}

impl When {
    pub fn subject_mut(&mut self) -> Option<&mut Expr> {
        match self {
            Self::Subject { subject, .. } => Some(subject),
            Self::Conditions { .. } => None,
        }
    }

    pub fn arms_mut(&mut self) -> impl Iterator<Item = WhenArmMut<'_>> {
        let (subject, conditions): (&mut [WhenArm], &mut [WhenArm<WhenPredicate>]) = match self {
            Self::Subject { arms, .. } => (arms, &mut []),
            Self::Conditions { arms, .. } => (&mut [], arms),
        };
        subject
            .iter_mut()
            .map(|arm| WhenArmMut {
                condition: match &mut arm.condition {
                    WhenCondition::Case(pattern) => WhenConditionMut::Case(pattern),
                    WhenCondition::Predicate(predicate) => WhenConditionMut::Predicate(predicate),
                    WhenCondition::Always => WhenConditionMut::Always,
                },
                guard: arm.guard.as_mut(),
                body: &mut arm.body,
            })
            .chain(conditions.iter_mut().map(|arm| WhenArmMut {
                condition: match &mut arm.condition {
                    WhenPredicate::Predicate(predicate) => WhenConditionMut::Predicate(predicate),
                    WhenPredicate::Always => WhenConditionMut::Always,
                },
                guard: arm.guard.as_mut(),
                body: &mut arm.body,
            }))
    }

    pub fn else_body_mut(&mut self) -> Option<&mut Vec<Statement>> {
        match self {
            Self::Subject {
                fallback: WhenFallback::Else(body),
                ..
            }
            | Self::Conditions {
                fallback: WhenFallback::Else(body),
                ..
            } => Some(body),
            _ => None,
        }
    }
}
