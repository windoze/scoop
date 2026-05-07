//! Codegen-stage gap inventory and backend gate helpers.
//!
//! This inventory is intentionally executable data: backend gates and tests consume the same
//! entries that document each `PIPELINE_GAPS.md` owner.  New unsupported codegen shapes should be
//! added here before they are allowed to reach LLVM body emission.

use crate::mir::{
    Body, MirCodegenBackendRoute, MirCodegenRoutingFact, Rvalue, StatementKind,
    StoredContinuationRoutePublication, TerminatorKind, UnwindAction,
};
use crate::span::Span;

use super::{LlvmEmitError, RefactorBackendGateError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CodegenGapRoute {
    RawMirLlvm,
    EffectRefactorLlvm,
    RuntimeC,
    FixtureRegression,
    UpstreamMirContract,
    FrontendReject,
}

impl CodegenGapRoute {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::RawMirLlvm => "raw MIR LLVM",
            Self::EffectRefactorLlvm => "effect-refactor LLVM",
            Self::RuntimeC => "runtime C",
            Self::FixtureRegression => "fixture/regression",
            Self::UpstreamMirContract => "upstream MIR contract",
            Self::FrontendReject => "frontend reject",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CodegenGapEntry {
    pub(crate) gap_id: &'static str,
    pub(crate) owner_task: &'static str,
    pub(crate) suggested_owner: &'static str,
    pub(crate) route: CodegenGapRoute,
    pub(crate) needs_upstream_contract: bool,
    pub(crate) production_blocker: bool,
    pub(crate) trigger: &'static str,
}

macro_rules! gap {
    ($gap_id:literal, $owner_task:literal, $suggested_owner:literal, $route:ident, $needs_upstream_contract:literal, $production_blocker:literal, $trigger:literal) => {
        CodegenGapEntry {
            gap_id: $gap_id,
            owner_task: $owner_task,
            suggested_owner: $suggested_owner,
            route: CodegenGapRoute::$route,
            needs_upstream_contract: $needs_upstream_contract,
            production_blocker: $production_blocker,
            trigger: $trigger,
        }
    };
}

pub(crate) const CODEGEN_GAP_INVENTORY: &[CodegenGapEntry] = &[
    gap!(
        "PIPELINE_GAPS §2.3",
        "MIR production verifier",
        "MIR-facing production verifier before CG-T01",
        UpstreamMirContract,
        true,
        true,
        "UnsupportedMainBody / pass MIR Todo"
    ),
    gap!(
        "PIPELINE_GAPS §3.1",
        "CG-T01",
        "CG-T01 / MIR-T12R",
        RawMirLlvm,
        true,
        true,
        "pass MIR terminator Handle/ResumeUnwind/Todo"
    ),
    gap!(
        "PIPELINE_GAPS §3.2",
        "CG-T01",
        "CG-T01 / MIR-T12R",
        RawMirLlvm,
        true,
        true,
        "pass MIR Perform cleanup unwind"
    ),
    gap!(
        "PIPELINE_GAPS §3.3",
        "CG-T01",
        "CG-T01 / MIR-T12R",
        RawMirLlvm,
        true,
        true,
        "pass MIR PerformResult default-value"
    ),
    gap!(
        "PIPELINE_GAPS §3.4",
        "CG-T02",
        "CG-T02 / MIR-T09R",
        RawMirLlvm,
        false,
        true,
        "pass MIR TypeCheck/Cast unsupported"
    ),
    gap!(
        "PIPELINE_GAPS §3.5",
        "CG-T02",
        "CG-T02 / MIR-T09R",
        EffectRefactorLlvm,
        false,
        true,
        "refactor value primitive runtime cast unsupported"
    ),
    gap!(
        "PIPELINE_GAPS §3.6",
        "CG-T01",
        "CG-T01 / MIR-T12R",
        RawMirLlvm,
        true,
        true,
        "pass MIR Virtual/Interface/Resume call kind"
    ),
    gap!(
        "PIPELINE_GAPS §3.7",
        "CG-T03",
        "CG-T03 / MIR-T07R",
        RawMirLlvm,
        true,
        true,
        "pass MIR TopLevelRef function reference"
    ),
    gap!(
        "PIPELINE_GAPS §3.8",
        "CG-T02",
        "CG-T02 / MIR-T09R",
        RawMirLlvm,
        false,
        true,
        "pass MIR pattern is Type"
    ),
    gap!(
        "PIPELINE_GAPS §3.9",
        "CG-T03",
        "CG-T03 / MIR-T08R",
        RawMirLlvm,
        true,
        true,
        "pass MIR class ctor default/named args"
    ),
    gap!(
        "PIPELINE_GAPS §3.10",
        "CG-T03",
        "CG-T03 / MIR-T07R",
        RawMirLlvm,
        true,
        true,
        "backend default-arg arity mismatch"
    ),
    gap!(
        "PIPELINE_GAPS §3.11",
        "CG-T04e",
        "CG-T04e / MIR-T10R",
        RawMirLlvm,
        false,
        true,
        "pass MIR closure env/aggregate shape"
    ),
    gap!(
        "PIPELINE_GAPS §3.12",
        "CG-T05",
        "CG-T05",
        EffectRefactorLlvm,
        false,
        true,
        "refactor effect-typed adapter unsupported"
    ),
    gap!(
        "PIPELINE_GAPS §3.13",
        "CG-T06",
        "CG-T06",
        UpstreamMirContract,
        true,
        true,
        "pass MIR ambiguous member continuation route"
    ),
    gap!(
        "PIPELINE_GAPS §4.1",
        "CG-T04b",
        "CG-T04b / MIR-T10R",
        EffectRefactorLlvm,
        false,
        true,
        "value boxing tuple/struct unsupported"
    ),
    gap!(
        "PIPELINE_GAPS §4.2",
        "CG-T04c",
        "CG-T04c / MIR-T10R",
        RawMirLlvm,
        false,
        true,
        "enum boxed payload field unit"
    ),
    gap!(
        "PIPELINE_GAPS §4.3",
        "CG-T04c",
        "CG-T04c / MIR-T10R",
        RawMirLlvm,
        false,
        true,
        "enum payload larger than word"
    ),
    gap!(
        "PIPELINE_GAPS §4.4",
        "CG-T04c",
        "CG-T04c / MIR-T10R",
        RawMirLlvm,
        false,
        true,
        "nested enum/tuple/struct payload unsupported"
    ),
    gap!(
        "PIPELINE_GAPS §4.5",
        "CG-T04d",
        "CG-T04d / MIR-T10R",
        EffectRefactorLlvm,
        false,
        true,
        "array composite element u64 word unsupported"
    ),
    gap!(
        "PIPELINE_GAPS §5.1",
        "CG-T05",
        "CG-T05",
        EffectRefactorLlvm,
        true,
        true,
        "refactor plain callable effect/control terminator"
    ),
    gap!(
        "PIPELINE_GAPS §5.2",
        "CG-T06",
        "CG-T06",
        EffectRefactorLlvm,
        true,
        true,
        "refactor unsupported source classification"
    ),
    gap!(
        "PIPELINE_GAPS §5.3",
        "CG-T06",
        "CG-T06",
        EffectRefactorLlvm,
        true,
        true,
        "refactor unwind payload/cleanup continuation contract missing"
    ),
    gap!(
        "PIPELINE_GAPS §5.4",
        "CG-T05",
        "CG-T05",
        EffectRefactorLlvm,
        true,
        true,
        "refactor effect-step main argv ABI unsupported"
    ),
    gap!(
        "PIPELINE_GAPS §5.5",
        "CG-T04f",
        "CG-T04f / MIR-T10R",
        RuntimeC,
        false,
        true,
        "runtime cross-thread resume u64 payload helper"
    ),
    gap!(
        "PIPELINE_GAPS §5.6",
        "CG-T06",
        "CG-T06",
        RuntimeC,
        true,
        true,
        "runtime fatal helper thread resume noncomplete"
    ),
    gap!(
        "PIPELINE_GAPS §5.7",
        "CG-T08",
        "CG-T08",
        FixtureRegression,
        false,
        true,
        "default refactor blocker regression"
    ),
    gap!(
        "PIPELINE_GAPS §6.1",
        "CG-T02",
        "CG-T02 / MIR-T09R",
        FixtureRegression,
        false,
        true,
        "not-null assertion !! expected-fail fixture"
    ),
    gap!(
        "PIPELINE_GAPS §6.2",
        "CG-T02",
        "CG-T02 / MIR-T09R",
        FixtureRegression,
        false,
        true,
        "runtime is/as/as? refactor path"
    ),
    gap!(
        "PIPELINE_GAPS §6.3",
        "CG-T03",
        "CG-T03",
        RawMirLlvm,
        false,
        true,
        "nameOf/getPlatform intrinsic fallback"
    ),
    gap!(
        "PIPELINE_GAPS §6.4",
        "CG-T07",
        "CG-T07",
        RawMirLlvm,
        true,
        true,
        "@Extern global storage/linkage"
    ),
    gap!(
        "PIPELINE_GAPS §6.5",
        "CG-T03",
        "CG-T03 / MIR-T08R",
        RawMirLlvm,
        true,
        false,
        "interface default method dispatch candidate"
    ),
    gap!(
        "PIPELINE_GAPS §7.2",
        "CG-T02",
        "CG-T02",
        FrontendReject,
        false,
        false,
        "function type runtime cast frontend diagnostic"
    ),
    gap!(
        "PIPELINE_GAPS §7.6",
        "CG-T07",
        "CG-T07",
        FrontendReject,
        true,
        false,
        "GC pin/handle intrinsic frontend diagnostic"
    ),
    gap!(
        "PIPELINE_GAPS §9",
        "CG-T08",
        "CG-T08",
        FixtureRegression,
        false,
        true,
        "codegen validation matrix"
    ),
];

pub(crate) fn codegen_gap_entry(gap_id: &str) -> Option<&'static CodegenGapEntry> {
    CODEGEN_GAP_INVENTORY
        .iter()
        .find(|entry| entry.gap_id == gap_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodegenBackendGateFailure {
    pub(crate) body_fqn: String,
    pub(crate) span: Span,
    pub(crate) detail: &'static str,
    pub(crate) entry: &'static CodegenGapEntry,
}

impl CodegenBackendGateFailure {
    pub(crate) fn into_llvm_error(self) -> LlvmEmitError {
        LlvmEmitError::RefactorBackendGate(Box::new(RefactorBackendGateError {
            body_fqn: self.body_fqn,
            source_span: self.span,
            gap_id: self.entry.gap_id,
            owner_task: self.entry.owner_task,
            suggested_owner: self.entry.suggested_owner,
            route: self.entry.route.as_str(),
            detail: self.detail,
            at: self.span.into(),
        }))
    }
}

pub(crate) fn raw_mir_backend_gate_failure(
    body_fqn: &str,
    body_span: Span,
    body: &Body,
    routing_fact: Option<&MirCodegenRoutingFact>,
    require_routing_fact: bool,
) -> Option<CodegenBackendGateFailure> {
    if let Some(failure) = raw_mir_body_shape_gate_failure(body_fqn, body_span, body) {
        return Some(failure);
    }

    if require_routing_fact {
        let Some(fact) = routing_fact else {
            return Some(gate_failure(
                body_fqn,
                body_span,
                "PIPELINE_GAPS §3.1",
                "raw MIR route attempted without a MIR-T12 codegen routing fact",
            ));
        };
        if !matches!(fact.route, MirCodegenBackendRoute::PlainRawMir) {
            return Some(gate_failure(
                body_fqn,
                body_span,
                raw_route_fact_gap_id(fact),
                raw_route_fact_detail(fact.route),
            ));
        }
    }

    None
}

fn raw_mir_body_shape_gate_failure(
    body_fqn: &str,
    body_span: Span,
    body: &Body,
) -> Option<CodegenBackendGateFailure> {
    for block in &body.blocks {
        for stmt in &block.stmts {
            match &stmt.kind {
                StatementKind::Todo(_) => {
                    return Some(gate_failure(
                        body_fqn,
                        stmt.span,
                        "PIPELINE_GAPS §2.3",
                        "MIR statement Todo reached raw LLVM emission without a production contract",
                    ));
                }
                StatementKind::Assign { value, .. } => {
                    if let Some(failure) = raw_mir_rvalue_gate_failure(body_fqn, stmt.span, value) {
                        return Some(failure);
                    }
                }
                StatementKind::StoreMember {
                    continuation_route: StoredContinuationRoutePublication::Ambiguous,
                    ..
                } => {
                    return Some(gate_failure(
                        body_fqn,
                        stmt.span,
                        "PIPELINE_GAPS §3.13",
                        "ambiguous member continuation route reached backend without a unique transport contract",
                    ));
                }
                StatementKind::Nop
                | StatementKind::StoreMember { .. }
                | StatementKind::StoreTopLevelVar { .. } => {}
            }
        }

        if let Some(failure) = raw_mir_unwind_gate_failure(
            body_fqn,
            block.terminator.span,
            &block.terminator.unwind,
            false,
        ) {
            return Some(failure);
        }

        match &block.terminator.kind {
            TerminatorKind::Todo(_) => {
                return Some(gate_failure(
                    body_fqn,
                    block.terminator.span,
                    "PIPELINE_GAPS §2.3",
                    "MIR terminator Todo reached raw LLVM emission without a production contract",
                ));
            }
            TerminatorKind::Handle { .. } | TerminatorKind::ResumeUnwind => {
                return Some(gate_failure(
                    body_fqn,
                    block.terminator.span,
                    "PIPELINE_GAPS §3.1",
                    "effect/control terminator reached raw LLVM emission without a plain-local or EffectStep handoff",
                ));
            }
            TerminatorKind::Perform { .. } => {
                if let Some(failure) = raw_mir_unwind_gate_failure(
                    body_fqn,
                    block.terminator.span,
                    &block.terminator.unwind,
                    true,
                ) {
                    return Some(failure);
                }
            }
            TerminatorKind::Return { .. }
            | TerminatorKind::Goto { .. }
            | TerminatorKind::CondBr { .. }
            | TerminatorKind::Unreachable => {}
        }
    }

    if body.blocks.is_empty() {
        return Some(gate_failure(
            body_fqn,
            body_span,
            "PIPELINE_GAPS §2.3",
            "empty MIR body reached raw LLVM emission before CFG validation",
        ));
    }

    None
}

fn raw_route_fact_gap_id(fact: &MirCodegenRoutingFact) -> &'static str {
    if fact
        .features
        .contains(&crate::mir::MirCodegenRouteFeature::PerformResult)
    {
        return "PIPELINE_GAPS §3.3";
    }
    if fact.features.iter().any(|feature| {
        matches!(
            feature,
            crate::mir::MirCodegenRouteFeature::VirtualCall
                | crate::mir::MirCodegenRouteFeature::InterfaceCall
                | crate::mir::MirCodegenRouteFeature::ResumeCall
        )
    }) {
        return "PIPELINE_GAPS §3.6";
    }
    "PIPELINE_GAPS §3.1"
}

fn raw_route_fact_detail(route: MirCodegenBackendRoute) -> &'static str {
    match route {
        MirCodegenBackendRoute::PlainRawMir => {
            "raw MIR route fact accepted a raw-safe effect-neutral body"
        }
        MirCodegenBackendRoute::PlainLocalControlHandoff => {
            "body is routed to plain-local control handoff; raw MIR route only accepts raw-safe effect-neutral bodies"
        }
        MirCodegenBackendRoute::EffectStepLowering => {
            "body is routed to EffectStep lowering; raw MIR route must not lower effect-control bodies"
        }
        MirCodegenBackendRoute::FrontendReject => {
            "body is routed to frontend rejection; raw MIR route must not emit rejected bodies"
        }
    }
}

fn raw_mir_rvalue_gate_failure(
    body_fqn: &str,
    span: Span,
    value: &Rvalue,
) -> Option<CodegenBackendGateFailure> {
    match value {
        Rvalue::Todo(_) => Some(gate_failure(
            body_fqn,
            span,
            "PIPELINE_GAPS §2.3",
            "MIR rvalue Todo reached raw LLVM emission without a production contract",
        )),
        Rvalue::PerformResult { .. } => Some(gate_failure(
            body_fqn,
            span,
            "PIPELINE_GAPS §3.3",
            "PerformResult reached raw LLVM emission without a published resume payload binding",
        )),
        Rvalue::Call { kind, .. } => match kind {
            crate::mir::CallKind::Virtual { .. }
            | crate::mir::CallKind::Interface { .. }
            | crate::mir::CallKind::Resume { .. } => Some(gate_failure(
                body_fqn,
                span,
                "PIPELINE_GAPS §3.6",
                "non-raw call kind reached raw LLVM emission without a routing fact",
            )),
            crate::mir::CallKind::Direct { .. }
            | crate::mir::CallKind::Closure { .. }
            | crate::mir::CallKind::FunValue { .. } => None,
        },
        Rvalue::ClassCtor { ctor, args, .. }
            if args.iter().any(|arg| arg.name.is_some())
                || args.len() != ctor.ordered_param_count =>
        {
            Some(gate_failure(
                body_fqn,
                span,
                "PIPELINE_GAPS §3.9",
                "class constructor selected/ordered argument contract reached backend incomplete",
            ))
        }
        Rvalue::Transport { .. } => Some(gate_failure(
            body_fqn,
            span,
            "PIPELINE_GAPS §4.1",
            "value erasure boxing transport reached raw LLVM emission before CG-T04b lowering",
        )),
        Rvalue::Use(_)
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
        | Rvalue::MakeClosure { .. } => None,
    }
}

fn raw_mir_unwind_gate_failure(
    body_fqn: &str,
    span: Span,
    unwind: &UnwindAction,
    is_perform_terminator: bool,
) -> Option<CodegenBackendGateFailure> {
    match unwind {
        UnwindAction::Todo(_) => Some(gate_failure(
            body_fqn,
            span,
            "PIPELINE_GAPS §5.3",
            "unwind action Todo reached backend without a published unwind/cleanup contract",
        )),
        UnwindAction::Cleanup { .. } if is_perform_terminator => Some(gate_failure(
            body_fqn,
            span,
            "PIPELINE_GAPS §3.2",
            "cleanup Perform reached raw LLVM emission without cleanup/resume routing",
        )),
        UnwindAction::NoUnwind | UnwindAction::Propagate | UnwindAction::Cleanup { .. } => None,
    }
}

fn gate_failure(
    body_fqn: &str,
    span: Span,
    gap_id: &'static str,
    detail: &'static str,
) -> CodegenBackendGateFailure {
    CodegenBackendGateFailure {
        body_fqn: body_fqn.to_string(),
        span,
        detail,
        entry: codegen_gap_entry(gap_id).expect("backend gate gap id must be in inventory"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::mir::{
        BasicBlock, BasicBlockId, MirCodegenAbiPublication, MirCodegenBackendRoute,
        MirCodegenRouteFeature, MirCodegenRoutingFact, Statement, Terminator,
    };
    use crate::ty::TypeStore;

    fn span() -> Span {
        Span::new(10, 4)
    }

    fn body_with_terminator(kind: TerminatorKind) -> Body {
        Body {
            locals: Vec::new(),
            start: BasicBlockId::from_raw(0),
            blocks: vec![BasicBlock {
                is_cleanup: false,
                stmts: Vec::new(),
                terminator: Terminator {
                    span: span(),
                    kind,
                    unwind: UnwindAction::NoUnwind,
                },
            }],
        }
    }

    fn body_with_assignment(value: Rvalue) -> Body {
        Body {
            locals: Vec::new(),
            start: BasicBlockId::from_raw(0),
            blocks: vec![BasicBlock {
                is_cleanup: false,
                stmts: vec![Statement {
                    span: span(),
                    kind: StatementKind::Assign {
                        target: crate::mir::LocalId::from_raw(0),
                        value,
                    },
                }],
                terminator: Terminator {
                    span: span(),
                    kind: TerminatorKind::Return {
                        value: Some(crate::mir::Operand::Const(crate::mir::ConstValue::Unit)),
                    },
                    unwind: UnwindAction::NoUnwind,
                },
            }],
        }
    }

    fn route_fact(
        route: MirCodegenBackendRoute,
        features: impl IntoIterator<Item = MirCodegenRouteFeature>,
    ) -> MirCodegenRoutingFact {
        MirCodegenRoutingFact {
            body_fqn: "sample.main".to_string(),
            span: span(),
            route,
            route_reason: "test route",
            features: features.into_iter().collect(),
            abi: MirCodegenAbiPublication::plain_no_outward(),
        }
    }

    #[test]
    fn codegen_gap_inventory_covers_pipeline_codegen_scope() {
        let expected = [
            "PIPELINE_GAPS §2.3",
            "PIPELINE_GAPS §3.1",
            "PIPELINE_GAPS §3.2",
            "PIPELINE_GAPS §3.3",
            "PIPELINE_GAPS §3.4",
            "PIPELINE_GAPS §3.5",
            "PIPELINE_GAPS §3.6",
            "PIPELINE_GAPS §3.7",
            "PIPELINE_GAPS §3.8",
            "PIPELINE_GAPS §3.9",
            "PIPELINE_GAPS §3.10",
            "PIPELINE_GAPS §3.11",
            "PIPELINE_GAPS §3.12",
            "PIPELINE_GAPS §3.13",
            "PIPELINE_GAPS §4.1",
            "PIPELINE_GAPS §4.2",
            "PIPELINE_GAPS §4.3",
            "PIPELINE_GAPS §4.4",
            "PIPELINE_GAPS §4.5",
            "PIPELINE_GAPS §5.1",
            "PIPELINE_GAPS §5.2",
            "PIPELINE_GAPS §5.3",
            "PIPELINE_GAPS §5.4",
            "PIPELINE_GAPS §5.5",
            "PIPELINE_GAPS §5.6",
            "PIPELINE_GAPS §5.7",
            "PIPELINE_GAPS §6.1",
            "PIPELINE_GAPS §6.2",
            "PIPELINE_GAPS §6.3",
            "PIPELINE_GAPS §6.4",
            "PIPELINE_GAPS §6.5",
            "PIPELINE_GAPS §7.2",
            "PIPELINE_GAPS §7.6",
            "PIPELINE_GAPS §9",
        ];

        let mut seen = BTreeSet::new();
        for entry in CODEGEN_GAP_INVENTORY {
            assert!(
                seen.insert(entry.gap_id),
                "duplicate gap id: {}",
                entry.gap_id
            );
            assert!(
                !entry.owner_task.is_empty(),
                "missing owner for {}",
                entry.gap_id
            );
            assert!(
                !entry.suggested_owner.is_empty(),
                "missing suggested owner for {}",
                entry.gap_id
            );
        }
        for gap_id in expected {
            assert!(
                codegen_gap_entry(gap_id).is_some(),
                "missing inventory entry for {gap_id}"
            );
        }
        assert_eq!(seen.len(), CODEGEN_GAP_INVENTORY.len());
    }

    #[test]
    fn codegen_gap_inventory_records_required_metadata() {
        for entry in CODEGEN_GAP_INVENTORY {
            assert!(entry.gap_id.starts_with("PIPELINE_GAPS §"));
            assert!(!entry.trigger.is_empty());
            if entry.needs_upstream_contract {
                assert!(
                    matches!(
                        entry.route,
                        CodegenGapRoute::RawMirLlvm
                            | CodegenGapRoute::EffectRefactorLlvm
                            | CodegenGapRoute::RuntimeC
                            | CodegenGapRoute::UpstreamMirContract
                            | CodegenGapRoute::FrontendReject
                    ),
                    "upstream contract need must be tied to a concrete route: {entry:?}"
                );
            }
        }
    }

    #[test]
    fn codegen_gap_inventory_keeps_composite_transport_owners_split() {
        for (gap_id, owner) in [
            ("PIPELINE_GAPS §3.11", "CG-T04e"),
            ("PIPELINE_GAPS §4.1", "CG-T04b"),
            ("PIPELINE_GAPS §4.2", "CG-T04c"),
            ("PIPELINE_GAPS §4.3", "CG-T04c"),
            ("PIPELINE_GAPS §4.4", "CG-T04c"),
            ("PIPELINE_GAPS §4.5", "CG-T04d"),
            ("PIPELINE_GAPS §5.5", "CG-T04f"),
        ] {
            let entry = codegen_gap_entry(gap_id).expect("composite gap must remain tracked");
            assert_eq!(entry.owner_task, owner, "{gap_id} owner drifted");
            assert_ne!(
                entry.owner_task, "CG-T04",
                "{gap_id} must not use a shared CG-T04 owner"
            );
        }
    }

    #[test]
    fn codegen_gap_inventory_covers_required_unsupported_patterns() {
        let triggers = CODEGEN_GAP_INVENTORY
            .iter()
            .map(|entry| entry.trigger)
            .collect::<Vec<_>>()
            .join("\n");

        for needle in [
            "UnsupportedMainBody",
            "pass MIR",
            "refactor value primitive runtime cast unsupported",
            "runtime fatal helper",
        ] {
            assert!(
                triggers.contains(needle),
                "inventory trigger list should cover `{needle}`"
            );
        }
    }

    #[test]
    fn refactor_llvm_backend_gate_rejects_missing_upstream_contract_before_body_emission() {
        let body = body_with_terminator(TerminatorKind::Todo("missing source contract"));
        let failure = raw_mir_backend_gate_failure("sample.main", span(), &body, None, false)
            .expect("gate should reject MIR Todo before raw body emission");

        assert_eq!(failure.body_fqn, "sample.main");
        assert_eq!(failure.entry.gap_id, "PIPELINE_GAPS §2.3");
        assert_eq!(failure.entry.owner_task, "MIR production verifier");
        assert!(failure.entry.needs_upstream_contract);

        let rendered = failure.into_llvm_error().to_string();
        assert!(rendered.contains("sample.main"));
        assert!(rendered.contains("PIPELINE_GAPS §2.3"));
        assert!(rendered.contains("MIR production verifier"));
    }

    #[test]
    fn refactor_llvm_backend_gate_allows_raw_safe_body() {
        let body = body_with_terminator(TerminatorKind::Return {
            value: Some(crate::mir::Operand::Const(crate::mir::ConstValue::Unit)),
        });

        assert!(raw_mir_backend_gate_failure("sample.main", span(), &body, None, false).is_none());
    }

    #[test]
    fn refactor_llvm_raw_route_gate_requires_published_routing_fact() {
        let body = body_with_terminator(TerminatorKind::Return {
            value: Some(crate::mir::Operand::Const(crate::mir::ConstValue::Unit)),
        });
        let failure = raw_mir_backend_gate_failure("sample.main", span(), &body, None, true)
            .expect("refactor raw route must consume MIR-T12 routing facts");

        assert_eq!(failure.entry.gap_id, "PIPELINE_GAPS §3.1");
        assert!(failure.detail.contains("MIR-T12 codegen routing fact"));
    }

    #[test]
    fn refactor_llvm_raw_route_gate_allows_raw_safe_body_with_plain_raw_fact() {
        let body = body_with_terminator(TerminatorKind::Return {
            value: Some(crate::mir::Operand::Const(crate::mir::ConstValue::Unit)),
        });
        let fact = route_fact(MirCodegenBackendRoute::PlainRawMir, []);

        assert!(
            raw_mir_backend_gate_failure("sample.main", span(), &body, Some(&fact), true).is_none()
        );
    }

    #[test]
    fn raw_mir_effect_control_route_rejects_plain_local_handoff_before_raw_emission() {
        let body = body_with_terminator(TerminatorKind::Return {
            value: Some(crate::mir::Operand::Const(crate::mir::ConstValue::Unit)),
        });
        let fact = route_fact(
            MirCodegenBackendRoute::PlainLocalControlHandoff,
            [MirCodegenRouteFeature::Perform],
        );
        let failure = raw_mir_backend_gate_failure("sample.main", span(), &body, Some(&fact), true)
            .expect("plain-local handoff bodies must not enter raw MIR emission");

        assert_eq!(failure.entry.gap_id, "PIPELINE_GAPS §3.1");
        assert!(failure.detail.contains("plain-local control handoff"));
    }

    #[test]
    fn raw_mir_effect_control_route_rejects_perform_result_default_value() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let body = body_with_assignment(Rvalue::PerformResult {
            op_fqn: "sample.Ping.hit".to_string(),
            effect_ty: builtins.unit,
        });
        let fact = route_fact(MirCodegenBackendRoute::PlainRawMir, []);
        let failure = raw_mir_backend_gate_failure("sample.main", span(), &body, Some(&fact), true)
            .expect("PerformResult must require a published resume payload binding");

        assert_eq!(failure.entry.gap_id, "PIPELINE_GAPS §3.3");
        assert!(failure.detail.contains("resume payload binding"));
    }
}
