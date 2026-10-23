use scoop_mir::optimization_report::*;
use serde_json::{Value, json};

pub(super) fn inline(outcome: InlineOutcome) -> &'static str {
    match outcome {
        InlineOutcome::Inlined => "inlined",
        InlineOutcome::ClassFields => "inlined-with-class-fields",
        InlineOutcome::Required => "required-expansion",
        InlineOutcome::ExecutionCost => "execution-cost",
        InlineOutcome::CallerGrowth => "caller-growth",
        InlineOutcome::IndirectCall => "indirect-call",
        InlineOutcome::NoBody => "no-mir-body",
        InlineOutcome::UnsupportedBody => "unsupported-body",
        InlineOutcome::RecursiveCallee => "recursive-callee",
        InlineOutcome::ProtocolCallee => "protocol-callee",
        InlineOutcome::ProtocolCaller => "protocol-caller",
        InlineOutcome::RecursiveExpansion => "recursive-expansion",
        InlineOutcome::Depth => "expansion-depth",
        InlineOutcome::GrowthExhausted => "growth-exhausted",
        InlineOutcome::ReleasePublication => "release-publication",
    }
}

pub(super) fn estimate(estimate: &InlineEstimate) -> Value {
    json!({
        "original": {"execution": estimate.original_execution, "size": estimate.original_size},
        "ordinary": cost(estimate.ordinary),
        "class_projection": match estimate.projection {
            InlineProjection::NotAttempted => json!({"status": "not-attempted"}),
            InlineProjection::Unproven => json!({"status": "unproven"}),
            InlineProjection::Estimated(estimate) => json!({"status": "estimated", "cost": cost(estimate)}),
        },
        "remaining_growth": estimate.remaining_growth, "in_loop": estimate.in_loop,
    })
}

fn cost(cost: InlineCost) -> Value {
    json!({"execution": cost.execution, "size": cost.size, "growth": cost.growth,
        "execution_limit": cost.execution_limit})
}

pub(super) fn allocation_kind(kind: AllocationKind) -> &'static str {
    match kind {
        AllocationKind::Box => "box",
        AllocationKind::Closure => "closure",
        AllocationKind::Class => "class",
    }
}

pub(super) fn allocation(outcome: AllocationOutcome) -> Value {
    let reason = match outcome {
        AllocationOutcome::Eliminated => "eliminated",
        AllocationOutcome::ExceptionFlow => "exception-flow",
        AllocationOutcome::ReleaseHook => "release-hook",
        AllocationOutcome::Representation => "class-representation",
        AllocationOutcome::AmbiguousSource => "ambiguous-source",
        AllocationOutcome::LoopCarried => "loop-carried-alias",
        AllocationOutcome::NotDominating => "creation-does-not-dominate-use",
        AllocationOutcome::UnreplacedUses {
            reads,
            replacements,
        } => {
            return json!({
                "reason": "unreplaced-uses", "reads": reads, "replacements": replacements,
            });
        }
    };
    json!({"reason": reason})
}
