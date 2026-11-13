use scoop_mir::optimization_report::*;
use serde_json::{Value, json};

pub(super) fn inline(outcome: InlineOutcome) -> &'static str {
    match outcome {
        InlineOutcome::Inlined => "inlined",
        InlineOutcome::ClassFields => "inlined-with-class-fields",
        InlineOutcome::KnownClosure => "inlined-known-closure",
        InlineOutcome::Required => "required-expansion",
        InlineOutcome::Profitability => "not-profitable",
        InlineOutcome::CallerGrowth => "caller-growth",
        InlineOutcome::IndirectCall => "indirect-call",
        InlineOutcome::NoBody => "no-mir-body",
        InlineOutcome::UnsupportedBody => "unsupported-body",
        InlineOutcome::RecursiveCallee => "recursive-callee",
        InlineOutcome::ProtocolCallee => "protocol-callee",
        InlineOutcome::RecursiveExpansion => "recursive-expansion",
        InlineOutcome::Depth => "expansion-depth",
        InlineOutcome::GrowthExhausted => "growth-exhausted",
        InlineOutcome::ReleasePublication => "release-publication",
    }
}

pub(super) fn estimate(estimate: &InlineEstimate) -> Value {
    json!({
        "original": {"execution": estimate.original_execution, "size": estimate.original_size},
        "baseline": {"execution": estimate.baseline_execution, "size": estimate.baseline_size},
        "ordinary": cost(estimate.ordinary),
        "class_projection": match estimate.projection {
            InlineProjection::NotAttempted => json!({"status": "not-attempted"}),
            InlineProjection::Unproven => json!({"status": "unproven"}),
            InlineProjection::Estimated(estimate) => json!({"status": "estimated", "cost": cost(estimate)}),
        },
        "remaining_growth": estimate.remaining_growth, "in_loop": estimate.in_loop,
        "region_remaining_growth": estimate.region_remaining_growth,
        "caller_remaining_growth": estimate.caller_remaining_growth,
        "heat": heat(estimate.heat),
        "caller_size": estimate.caller_size, "result_used": estimate.result_used,
        "arguments": {"constant": estimate.constant_arguments, "exact_type": estimate.exact_arguments},
    })
}

pub(super) fn heat(heat: InlineHeat) -> Value {
    json!({
        "source": match heat.source {
            InlineHeatSource::Ordinary => "ordinary",
            InlineHeatSource::Loop => "loop",
            InlineHeatSource::ColdExit => "cold-exit",
            InlineHeatSource::Profile => "profile",
        },
        "loop_depth": heat.loop_depth,
        "loop_header": heat.loop_header.map(|id| id.into_raw().into_u32()),
        "numerator": heat.numerator, "denominator": heat.denominator,
        "profile": heat.profile.map(|profile| json!({
            "location": profile.location,
            "frequency": match profile.frequency {
                Ok(value) => json!({"calls": value.calls, "entries": value.entries,
                    "partial": value.partial}),
                Err(reason) => json!({"unavailable": missing(reason)}),
            },
        })),
    })
}

fn missing(reason: scoop_profile::MissingCount) -> &'static str {
    use scoop_profile::MissingCount::*;
    match reason {
        MissingBody => "missing-body",
        BodyChanged => "body-changed",
        NotEmitted => "not-emitted",
        MissingRecord => "missing-record",
        ZeroEntry => "zero-entry",
        InconsistentFlow => "inconsistent-flow",
        Overflow => "overflow",
    }
}

fn cost(cost: InlineCost) -> Value {
    json!({"execution": cost.execution, "size": cost.size,
        "growth": cost.growth, "net_growth": cost.net_growth, "complexity": cost.complexity,
        "benefit": {"call": cost.benefit.call, "abi": cost.benefit.abi,
            "simplification": cost.benefit.simplification, "projection": cost.benefit.projection},
        "weighted_benefit": cost.weighted_benefit, "penalty": cost.penalty})
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
