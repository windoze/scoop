use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use thiserror::Error;

use crate::span::Span;

use super::{
    CallKind, FunDecl, MirCallableAbiKind, MirCallableImplPlan, Rvalue, StatementKind,
    TerminatorKind,
};

/// Codegen-visible features discovered in a materialized MIR callable body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MirCodegenRouteFeature {
    Handle,
    ResumeUnwind,
    Perform,
    PerformResult,
    VirtualCall,
    InterfaceCall,
    ResumeCall,
}

impl MirCodegenRouteFeature {
    fn raw_mir_unsupported(self) -> bool {
        matches!(
            self,
            Self::Handle
                | Self::ResumeUnwind
                | Self::Perform
                | Self::PerformResult
                | Self::VirtualCall
                | Self::InterfaceCall
                | Self::ResumeCall
        )
    }
}

/// Backend route that a materialized callable is allowed to enter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirCodegenBackendRoute {
    PlainRawMir,
    PlainLocalControlHandoff,
    EffectStepLowering,
    FrontendReject,
}

/// Final ABI publication for a materialized callable as consumed by codegen routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirCodegenAbiPublication {
    pub callable_abi_kind: MirCallableAbiKind,
    pub resolved_outward_cases: Vec<String>,
    pub impl_plan: MirCallableImplPlan,
    pub adapter_required: bool,
    pub step_schema_published: bool,
}

impl MirCodegenAbiPublication {
    pub fn plain_no_outward() -> Self {
        Self {
            callable_abi_kind: MirCallableAbiKind::Plain,
            resolved_outward_cases: Vec::new(),
            impl_plan: MirCallableImplPlan::NoOutward,
            adapter_required: false,
            step_schema_published: false,
        }
    }
}

/// Per-callable MIR-to-codegen routing fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirCodegenRoutingFact {
    pub body_fqn: String,
    pub span: Span,
    pub route: MirCodegenBackendRoute,
    pub route_reason: &'static str,
    pub features: BTreeSet<MirCodegenRouteFeature>,
    pub abi: MirCodegenAbiPublication,
}

impl MirCodegenRoutingFact {
    pub fn from_materialized_fun(fun: &FunDecl, abi: MirCodegenAbiPublication) -> Self {
        let features = collect_route_features(fun);
        Self::new(fun.fqn.clone(), fun.span, features, abi)
    }

    pub fn declaration_only(
        body_fqn: impl Into<String>,
        span: Span,
        abi: MirCodegenAbiPublication,
    ) -> Self {
        Self::new(body_fqn.into(), span, BTreeSet::new(), abi)
    }

    pub fn new(
        body_fqn: String,
        span: Span,
        features: BTreeSet<MirCodegenRouteFeature>,
        abi: MirCodegenAbiPublication,
    ) -> Self {
        let (route, route_reason) = choose_route(&features, &abi);
        Self {
            body_fqn,
            span,
            route,
            route_reason,
            features,
            abi,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MirCodegenRoutingFacts {
    by_body_fqn: BTreeMap<String, MirCodegenRoutingFact>,
}

impl MirCodegenRoutingFacts {
    pub fn new(facts: impl IntoIterator<Item = MirCodegenRoutingFact>) -> Self {
        let by_body_fqn = facts
            .into_iter()
            .map(|fact| (fact.body_fqn.clone(), fact))
            .collect();
        Self { by_body_fqn }
    }

    pub fn get(&self, body_fqn: &str) -> Option<&MirCodegenRoutingFact> {
        self.by_body_fqn.get(body_fqn)
    }

    pub fn iter(&self) -> impl Iterator<Item = &MirCodegenRoutingFact> {
        self.by_body_fqn.values()
    }

    pub fn validate(&self) -> Result<(), MirCodegenRouteError> {
        for fact in self.iter() {
            validate_fact(fact)?;
        }
        Ok(())
    }

    pub fn stable_dump(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "codegen_routing_facts:").unwrap();
        if self.by_body_fqn.is_empty() {
            writeln!(&mut out, "  <none>").unwrap();
            return out;
        }
        for fact in self.iter() {
            writeln!(&mut out, "  - body: {}", fact.body_fqn).unwrap();
            writeln!(&mut out, "    route: {:?}", fact.route).unwrap();
            writeln!(&mut out, "    reason: {}", fact.route_reason).unwrap();
            writeln!(
                &mut out,
                "    abi: {:?} impl_plan={:?} resolved_outward_cases=[{}] adapter_required={} step_schema_published={}",
                fact.abi.callable_abi_kind,
                fact.abi.impl_plan,
                fact.abi.resolved_outward_cases.join(", "),
                fact.abi.adapter_required,
                fact.abi.step_schema_published,
            )
            .unwrap();
            writeln!(
                &mut out,
                "    features: {}",
                render_features(&fact.features)
            )
            .unwrap();
            writeln!(&mut out, "    span: {:?}", fact.span).unwrap();
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MirCodegenRouteError {
    #[error("codegen routing facts are missing callable facts for materialized body `{body_fqn}`")]
    MissingCallableFacts { body_fqn: String },

    #[error(
        "codegen route for `{body_fqn}` at {span:?} allows raw MIR but body contains unsupported feature {feature:?}"
    )]
    RawRouteUnsupportedFeature {
        body_fqn: String,
        span: Span,
        feature: MirCodegenRouteFeature,
    },

    #[error("codegen route for `{body_fqn}` at {span:?} has invalid ABI publication: {detail}")]
    InvalidAbiPublication {
        body_fqn: String,
        span: Span,
        detail: &'static str,
    },

    #[error("codegen route for `{body_fqn}` at {span:?} requires frontend rejection: {reason}")]
    FrontendRejectRoute {
        body_fqn: String,
        span: Span,
        reason: &'static str,
    },
}

fn choose_route(
    features: &BTreeSet<MirCodegenRouteFeature>,
    abi: &MirCodegenAbiPublication,
) -> (MirCodegenBackendRoute, &'static str) {
    match abi.callable_abi_kind {
        MirCallableAbiKind::EffectStep => (
            MirCodegenBackendRoute::EffectStepLowering,
            "callable publishes EffectStep ABI from non-empty outward cases",
        ),
        MirCallableAbiKind::Plain | MirCallableAbiKind::DeferredToEffectFacts => {
            if features.iter().any(|feature| feature.raw_mir_unsupported()) {
                (
                    MirCodegenBackendRoute::PlainLocalControlHandoff,
                    "plain ABI body contains local control or non-raw call-site features",
                )
            } else {
                (
                    MirCodegenBackendRoute::PlainRawMir,
                    "plain ABI body is raw-MIR safe",
                )
            }
        }
    }
}

fn validate_fact(fact: &MirCodegenRoutingFact) -> Result<(), MirCodegenRouteError> {
    if matches!(fact.route, MirCodegenBackendRoute::FrontendReject) {
        return Err(MirCodegenRouteError::FrontendRejectRoute {
            body_fqn: fact.body_fqn.clone(),
            span: fact.span,
            reason: fact.route_reason,
        });
    }

    if matches!(fact.route, MirCodegenBackendRoute::PlainRawMir)
        && let Some(feature) = fact
            .features
            .iter()
            .copied()
            .find(|feature| feature.raw_mir_unsupported())
    {
        return Err(MirCodegenRouteError::RawRouteUnsupportedFeature {
            body_fqn: fact.body_fqn.clone(),
            span: fact.span,
            feature,
        });
    }

    match fact.abi.callable_abi_kind {
        MirCallableAbiKind::DeferredToEffectFacts => Err(invalid_abi(
            fact,
            "codegen routing requires finalized effect facts, not deferred ABI markers",
        )),
        MirCallableAbiKind::Plain => {
            if fact.abi.impl_plan != MirCallableImplPlan::NoOutward {
                return Err(invalid_abi(
                    fact,
                    "plain ABI body must publish impl_plan=NoOutward",
                ));
            }
            if !fact.abi.resolved_outward_cases.is_empty() {
                return Err(invalid_abi(
                    fact,
                    "plain ABI body must not publish outward cases",
                ));
            }
            if fact.abi.step_schema_published {
                return Err(invalid_abi(
                    fact,
                    "plain ABI body must not publish a body Step schema",
                ));
            }
            if matches!(fact.route, MirCodegenBackendRoute::EffectStepLowering) {
                return Err(invalid_abi(
                    fact,
                    "plain ABI body cannot use EffectStep lowering route",
                ));
            }
            Ok(())
        }
        MirCallableAbiKind::EffectStep => {
            if fact.abi.impl_plan == MirCallableImplPlan::NoOutward {
                return Err(invalid_abi(
                    fact,
                    "NoOutward body must publish plain ABI, not EffectStep ABI",
                ));
            }
            if fact.abi.resolved_outward_cases.is_empty() {
                return Err(invalid_abi(
                    fact,
                    "EffectStep body ABI requires non-empty resolved outward cases",
                ));
            }
            if !fact.abi.step_schema_published {
                return Err(invalid_abi(
                    fact,
                    "EffectStep ABI requires a published body Step schema",
                ));
            }
            if !matches!(fact.route, MirCodegenBackendRoute::EffectStepLowering) {
                return Err(invalid_abi(
                    fact,
                    "EffectStep ABI must use the EffectStep lowering route",
                ));
            }
            Ok(())
        }
    }
}

fn invalid_abi(fact: &MirCodegenRoutingFact, detail: &'static str) -> MirCodegenRouteError {
    MirCodegenRouteError::InvalidAbiPublication {
        body_fqn: fact.body_fqn.clone(),
        span: fact.span,
        detail,
    }
}

fn collect_route_features(fun: &FunDecl) -> BTreeSet<MirCodegenRouteFeature> {
    let mut features = BTreeSet::new();
    let Some(body) = &fun.body else {
        return features;
    };
    for block in &body.blocks {
        for stmt in &block.stmts {
            if let StatementKind::Assign { value, .. } = &stmt.kind {
                collect_rvalue_features(value, &mut features);
            }
        }
        match &block.terminator.kind {
            TerminatorKind::Handle { .. } => {
                features.insert(MirCodegenRouteFeature::Handle);
            }
            TerminatorKind::ResumeUnwind => {
                features.insert(MirCodegenRouteFeature::ResumeUnwind);
            }
            TerminatorKind::Perform { .. } => {
                features.insert(MirCodegenRouteFeature::Perform);
            }
            TerminatorKind::Return { .. }
            | TerminatorKind::Goto { .. }
            | TerminatorKind::CondBr { .. }
            | TerminatorKind::Unreachable
            | TerminatorKind::Todo(_) => {}
        }
    }
    features
}

fn collect_rvalue_features(value: &Rvalue, features: &mut BTreeSet<MirCodegenRouteFeature>) {
    match value {
        Rvalue::Call { kind, .. } => match kind {
            CallKind::Virtual { .. } => {
                features.insert(MirCodegenRouteFeature::VirtualCall);
            }
            CallKind::Interface { .. } => {
                features.insert(MirCodegenRouteFeature::InterfaceCall);
            }
            CallKind::Resume { .. } => {
                features.insert(MirCodegenRouteFeature::ResumeCall);
            }
            CallKind::Direct { .. } | CallKind::Closure { .. } | CallKind::FunValue { .. } => {}
        },
        Rvalue::PerformResult { .. } => {
            features.insert(MirCodegenRouteFeature::PerformResult);
        }
        Rvalue::Use(_)
        | Rvalue::Transport { .. }
        | Rvalue::TopLevelRef(_)
        | Rvalue::UnresolvedName { .. }
        | Rvalue::Unary { .. }
        | Rvalue::Binary { .. }
        | Rvalue::TypeCheck { .. }
        | Rvalue::Cast { .. }
        | Rvalue::MemberAccess { .. }
        | Rvalue::EnumVariant { .. }
        | Rvalue::ClassCtor { .. }
        | Rvalue::MakeTuple { .. }
        | Rvalue::StructLit { .. }
        | Rvalue::SizeOf { .. }
        | Rvalue::TypeMetadataLiteral(_)
        | Rvalue::InterpolatedString { .. }
        | Rvalue::TupleGet { .. }
        | Rvalue::CaptureBoxNew { .. }
        | Rvalue::CaptureBoxGet { .. }
        | Rvalue::CaptureBoxSet { .. }
        | Rvalue::PatternMatch { .. }
        | Rvalue::PatternExtract { .. }
        | Rvalue::MakeClosure { .. }
        | Rvalue::Todo(_) => {}
    }
}

fn render_features(features: &BTreeSet<MirCodegenRouteFeature>) -> String {
    if features.is_empty() {
        return "<none>".to_string();
    }
    features
        .iter()
        .map(|feature| format!("{feature:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Span;

    fn span() -> Span {
        Span::new(1, 2)
    }

    fn fact_with(
        route: MirCodegenBackendRoute,
        features: impl IntoIterator<Item = MirCodegenRouteFeature>,
        abi: MirCodegenAbiPublication,
    ) -> MirCodegenRoutingFact {
        MirCodegenRoutingFact {
            body_fqn: "sample.main".to_string(),
            span: span(),
            route,
            route_reason: "test route",
            features: features.into_iter().collect(),
            abi,
        }
    }

    #[test]
    fn refactor_materialized_mir_codegen_route_verifier_accepts_raw_safe_plain_body() {
        let facts = MirCodegenRoutingFacts::new([fact_with(
            MirCodegenBackendRoute::PlainRawMir,
            [],
            MirCodegenAbiPublication::plain_no_outward(),
        )]);

        facts.validate().unwrap();
    }

    #[test]
    fn refactor_materialized_mir_codegen_route_verifier_rejects_raw_route_unsupported_feature() {
        let facts = MirCodegenRoutingFacts::new([fact_with(
            MirCodegenBackendRoute::PlainRawMir,
            [MirCodegenRouteFeature::VirtualCall],
            MirCodegenAbiPublication::plain_no_outward(),
        )]);

        let err = facts.validate().unwrap_err();
        assert!(matches!(
            err,
            MirCodegenRouteError::RawRouteUnsupportedFeature {
                feature: MirCodegenRouteFeature::VirtualCall,
                ..
            }
        ));
    }

    #[test]
    fn refactor_materialized_mir_codegen_route_verifier_rejects_no_outward_effect_step_abi() {
        let facts = MirCodegenRoutingFacts::new([fact_with(
            MirCodegenBackendRoute::EffectStepLowering,
            [],
            MirCodegenAbiPublication {
                callable_abi_kind: MirCallableAbiKind::EffectStep,
                resolved_outward_cases: Vec::new(),
                impl_plan: MirCallableImplPlan::NoOutward,
                adapter_required: false,
                step_schema_published: true,
            },
        )]);

        let err = facts.validate().unwrap_err();
        assert!(matches!(
            err,
            MirCodegenRouteError::InvalidAbiPublication { detail, .. }
                if detail.contains("NoOutward")
        ));
    }

    #[test]
    fn refactor_materialized_mir_codegen_route_verifier_rejects_frontend_reject_route() {
        let facts = MirCodegenRoutingFacts::new([fact_with(
            MirCodegenBackendRoute::FrontendReject,
            [],
            MirCodegenAbiPublication::plain_no_outward(),
        )]);

        assert!(matches!(
            facts.validate().unwrap_err(),
            MirCodegenRouteError::FrontendRejectRoute { .. }
        ));
    }
}
