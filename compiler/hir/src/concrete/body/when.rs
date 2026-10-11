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

pub use crate::ExhaustivenessProof;

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
