//! Codegen-stage gap inventory and backend gate helpers.
//!
//! This inventory is intentionally executable data: backend gates and tests consume the same
//! entries that document each `PIPELINE_GAPS.md` owner.  New unsupported codegen shapes should be
//! added here before they are allowed to reach LLVM body emission.

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
        "P2-T03",
        "P2-T03 / upstream impossible-state guard",
        UpstreamMirContract,
        true,
        false,
        "UnsupportedMainBody / production MIR contract guard / pass MIR Todo"
    ),
    gap!(
        "PIPELINE_GAPS §3.1",
        "P3-T01",
        "P3-T01 / raw-route gate guard",
        RawMirLlvm,
        true,
        false,
        "backend gate / raw MIR effect-control terminator route bug"
    ),
    gap!(
        "PIPELINE_GAPS §3.2",
        "P3-T01",
        "P3-T01 / raw-route gate guard",
        RawMirLlvm,
        true,
        false,
        "backend gate / raw MIR Perform route bug"
    ),
    gap!(
        "PIPELINE_GAPS §3.3",
        "P3-T01",
        "P3-T01 / raw-route gate guard",
        RawMirLlvm,
        true,
        false,
        "backend gate / raw MIR PerformResult route bug"
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
        "P3-T01",
        "P3-T01 / raw-route gate guard",
        RawMirLlvm,
        true,
        false,
        "backend gate / raw MIR missing dispatch/resume handoff contract"
    ),
    gap!(
        "PIPELINE_GAPS §3.7",
        "P3-T03",
        "P3-T03 / top-level callable regression guard",
        FixtureRegression,
        false,
        false,
        "top-level callable value / FunPtr regression guard"
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
        "P3-T02",
        "P3-T02 / typed ctor contract guard",
        UpstreamMirContract,
        true,
        false,
        "typed class ctor selected/ordered args contract drift"
    ),
    gap!(
        "PIPELINE_GAPS §3.10",
        "P3-T02",
        "P3-T02 / typed default-arg contract guard",
        UpstreamMirContract,
        true,
        false,
        "typed default-arg ordered call contract drift"
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
        "P4-T01",
        "P4-T01 / effect-typed callable adapter regression guard",
        EffectRefactorLlvm,
        false,
        false,
        "effect-typed callable adapter regression guard"
    ),
    gap!(
        "PIPELINE_GAPS §3.13",
        "P3-T03",
        "P3-T03 / StoreMember continuation contract guard",
        UpstreamMirContract,
        true,
        false,
        "typed StoreMember continuation route must be unique before handoff"
    ),
    gap!(
        "PIPELINE_GAPS §4.1",
        "P5-T01",
        "P5-T01 / composite value erasure guard",
        EffectRefactorLlvm,
        true,
        false,
        "composite value erasure must publish descriptor-backed boxing intent"
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
        "P5-T01",
        "P5-T01 / oversized enum payload boxing guard",
        RawMirLlvm,
        false,
        false,
        "oversized enum payload must already route through boxed composite transport"
    ),
    gap!(
        "PIPELINE_GAPS §4.4",
        "P5-T01",
        "P5-T01 / boxed enum payload guard",
        RawMirLlvm,
        false,
        false,
        "boxed enum payload contract must cover nested enum/tuple/struct payloads"
    ),
    gap!(
        "PIPELINE_GAPS §4.5",
        "P5-T01",
        "P5-T01 / array composite element transport guard",
        EffectRefactorLlvm,
        true,
        false,
        "array composite element transport must publish descriptor-backed metadata"
    ),
    gap!(
        "PIPELINE_GAPS §5.1",
        "P4-T01",
        "P4-T01 / actual-outward ABI routing guard",
        EffectRefactorLlvm,
        true,
        false,
        "actual outward effect set must uniquely decide callable ABI"
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
        "P4-T02",
        "P4-T02 / cleanup-unwind contract guard",
        EffectRefactorLlvm,
        true,
        false,
        "published ResumeUnwind cleanup contract must remain coherent"
    ),
    gap!(
        "PIPELINE_GAPS §5.4",
        "P4-T01",
        "P4-T01 / outward-empty plain routing guard",
        EffectRefactorLlvm,
        true,
        false,
        "outward-empty callable must publish plain entry routing"
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

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
    fn codegen_gap_inventory_marks_p5_t01_composite_transport_gaps_as_closed_guards() {
        for (gap_id, suggested_owner, route, needs_upstream_contract, trigger) in [
            (
                "PIPELINE_GAPS §4.1",
                "P5-T01 / composite value erasure guard",
                CodegenGapRoute::EffectRefactorLlvm,
                true,
                "composite value erasure must publish descriptor-backed boxing intent",
            ),
            (
                "PIPELINE_GAPS §4.3",
                "P5-T01 / oversized enum payload boxing guard",
                CodegenGapRoute::RawMirLlvm,
                false,
                "oversized enum payload must already route through boxed composite transport",
            ),
            (
                "PIPELINE_GAPS §4.4",
                "P5-T01 / boxed enum payload guard",
                CodegenGapRoute::RawMirLlvm,
                false,
                "boxed enum payload contract must cover nested enum/tuple/struct payloads",
            ),
            (
                "PIPELINE_GAPS §4.5",
                "P5-T01 / array composite element transport guard",
                CodegenGapRoute::EffectRefactorLlvm,
                true,
                "array composite element transport must publish descriptor-backed metadata",
            ),
        ] {
            let entry = codegen_gap_entry(gap_id)
                .unwrap_or_else(|| panic!("{gap_id} guard should remain tracked in inventory"));
            assert_eq!(entry.owner_task, "P5-T01");
            assert_eq!(entry.suggested_owner, suggested_owner);
            assert_eq!(entry.route, route);
            assert_eq!(entry.needs_upstream_contract, needs_upstream_contract);
            assert!(
                !entry.production_blocker,
                "{gap_id} should now be represented as a closed composite-transport guard"
            );
            assert_eq!(entry.trigger, trigger);
        }
    }

    #[test]
    fn codegen_gap_inventory_marks_2_3_as_nonblocking_upstream_guard() {
        let entry = codegen_gap_entry("PIPELINE_GAPS §2.3")
            .expect("§2.3 guard should remain tracked in inventory");
        assert_eq!(entry.owner_task, "P2-T03");
        assert_eq!(entry.route, CodegenGapRoute::UpstreamMirContract);
        assert!(entry.needs_upstream_contract);
        assert!(
            !entry.production_blocker,
            "§2.3 should now be a guard-only upstream contract bucket"
        );
        assert!(
            entry.trigger.contains("pass MIR Todo"),
            "§2.3 trigger should keep pointing at downstream Todo guard"
        );
    }

    #[test]
    fn codegen_gap_inventory_marks_p3_t01_raw_route_gaps_as_nonblocking_guards() {
        for (gap_id, trigger) in [
            (
                "PIPELINE_GAPS §3.1",
                "backend gate / raw MIR effect-control terminator route bug",
            ),
            (
                "PIPELINE_GAPS §3.2",
                "backend gate / raw MIR Perform route bug",
            ),
            (
                "PIPELINE_GAPS §3.3",
                "backend gate / raw MIR PerformResult route bug",
            ),
            (
                "PIPELINE_GAPS §3.6",
                "backend gate / raw MIR missing dispatch/resume handoff contract",
            ),
        ] {
            let entry = codegen_gap_entry(gap_id)
                .unwrap_or_else(|| panic!("{gap_id} guard should remain tracked in inventory"));
            assert_eq!(entry.owner_task, "P3-T01");
            assert_eq!(entry.route, CodegenGapRoute::RawMirLlvm);
            assert!(entry.needs_upstream_contract);
            assert!(
                !entry.production_blocker,
                "{gap_id} should now be represented as a raw-route gate guard"
            );
            assert_eq!(entry.trigger, trigger);
        }
    }

    #[test]
    fn codegen_gap_inventory_marks_p3_t03_regression_and_storemember_gaps_as_closed_guards() {
        let function_ref = codegen_gap_entry("PIPELINE_GAPS §3.7")
            .expect("§3.7 regression guard should remain tracked in inventory");
        assert_eq!(function_ref.owner_task, "P3-T03");
        assert_eq!(function_ref.route, CodegenGapRoute::FixtureRegression);
        assert!(!function_ref.needs_upstream_contract);
        assert!(!function_ref.production_blocker);
        assert_eq!(
            function_ref.trigger,
            "top-level callable value / FunPtr regression guard"
        );

        let store_member = codegen_gap_entry("PIPELINE_GAPS §3.13")
            .expect("§3.13 guard should remain tracked in inventory");
        assert_eq!(store_member.owner_task, "P3-T03");
        assert_eq!(store_member.route, CodegenGapRoute::UpstreamMirContract);
        assert!(store_member.needs_upstream_contract);
        assert!(!store_member.production_blocker);
        assert_eq!(
            store_member.trigger,
            "typed StoreMember continuation route must be unique before handoff"
        );
    }

    #[test]
    fn codegen_gap_inventory_marks_p4_t01_effect_routing_gaps_as_closed_guards() {
        for (gap_id, owner, suggested_owner, needs_upstream_contract, trigger) in [
            (
                "PIPELINE_GAPS §3.12",
                "P4-T01",
                "P4-T01 / effect-typed callable adapter regression guard",
                false,
                "effect-typed callable adapter regression guard",
            ),
            (
                "PIPELINE_GAPS §5.1",
                "P4-T01",
                "P4-T01 / actual-outward ABI routing guard",
                true,
                "actual outward effect set must uniquely decide callable ABI",
            ),
            (
                "PIPELINE_GAPS §5.4",
                "P4-T01",
                "P4-T01 / outward-empty plain routing guard",
                true,
                "outward-empty callable must publish plain entry routing",
            ),
        ] {
            let entry = codegen_gap_entry(gap_id)
                .unwrap_or_else(|| panic!("{gap_id} guard should remain tracked in inventory"));
            assert_eq!(entry.owner_task, owner);
            assert_eq!(entry.suggested_owner, suggested_owner);
            assert_eq!(entry.route, CodegenGapRoute::EffectRefactorLlvm);
            assert_eq!(entry.needs_upstream_contract, needs_upstream_contract);
            assert!(
                !entry.production_blocker,
                "{gap_id} should now be represented as a closed effect-routing guard"
            );
            assert_eq!(entry.trigger, trigger);
        }
    }

    #[test]
    fn codegen_gap_inventory_marks_p4_t02_cleanup_unwind_gap_as_closed_guard() {
        let entry = codegen_gap_entry("PIPELINE_GAPS §5.3")
            .expect("§5.3 guard should remain tracked in inventory");
        assert_eq!(entry.owner_task, "P4-T02");
        assert_eq!(
            entry.suggested_owner,
            "P4-T02 / cleanup-unwind contract guard"
        );
        assert_eq!(entry.route, CodegenGapRoute::EffectRefactorLlvm);
        assert!(entry.needs_upstream_contract);
        assert!(
            !entry.production_blocker,
            "§5.3 should now be represented as a closed cleanup/unwind guard"
        );
        assert_eq!(
            entry.trigger,
            "published ResumeUnwind cleanup contract must remain coherent"
        );
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
}
