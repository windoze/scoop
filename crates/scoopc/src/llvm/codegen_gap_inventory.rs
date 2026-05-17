//! Codegen-stage gap inventory and backend gate helpers.
//!
//! This inventory is intentionally executable data: only gap ids that still back an executable
//! backend guard or a codegen-adjacent frontend gate stay here. Pure regression coverage for
//! already-closed surfaces lives in dedicated fixtures / IR tests instead of this active table.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CodegenGapRoute {
    RawMirLlvm,
    EffectLoweredLlvm,
    UpstreamMirContract,
    FrontendReject,
}

impl CodegenGapRoute {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::RawMirLlvm => "raw MIR LLVM",
            Self::EffectLoweredLlvm => "effect-lowered LLVM",
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
        "PIPELINE_GAPS §3.5",
        "P6-T01",
        "P6-T01 / runtime cast contract guard",
        EffectLoweredLlvm,
        true,
        false,
        "runtime cast/typecheck metadata must stay within supported runtime-ref or static-folded surface"
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
        "PIPELINE_GAPS §3.8",
        "P5-T02",
        "P5-T02 / pattern type-test gate and guard",
        FrontendReject,
        false,
        false,
        "when-pattern runtime type test target must stay in ref/string or statically folded value surface"
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
        "P5-T02",
        "P5-T02 / closure env composite transport guard",
        EffectLoweredLlvm,
        true,
        false,
        "closure env transport must publish shared descriptor-backed env/capture contract"
    ),
    gap!(
        "PIPELINE_GAPS §3.12",
        "P4-T01",
        "P4-T01 / effect-typed callable adapter regression guard",
        EffectLoweredLlvm,
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
        EffectLoweredLlvm,
        true,
        false,
        "composite value erasure must publish descriptor-backed boxing intent"
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
        EffectLoweredLlvm,
        true,
        false,
        "array composite element transport must publish descriptor-backed metadata"
    ),
    gap!(
        "PIPELINE_GAPS §5.1",
        "P4-T01",
        "P4-T01 / actual-outward ABI routing guard",
        EffectLoweredLlvm,
        true,
        false,
        "actual outward effect set must uniquely decide callable ABI"
    ),
    gap!(
        "PIPELINE_GAPS §5.3",
        "P4-T02",
        "P4-T02 / cleanup-unwind contract guard",
        EffectLoweredLlvm,
        true,
        false,
        "published ResumeUnwind cleanup contract must remain coherent"
    ),
    gap!(
        "PIPELINE_GAPS §5.4",
        "P4-T01",
        "P4-T01 / outward-empty plain routing guard",
        EffectLoweredLlvm,
        true,
        false,
        "outward-empty callable must publish plain entry routing"
    ),
    gap!(
        "PIPELINE_GAPS §7.2",
        "P6-T02",
        "P6-T02 / function-type runtime cast frontend gate",
        FrontendReject,
        false,
        false,
        "function-type runtime cast must fail at the frontend until a real runtime cast contract exists"
    ),
    gap!(
        "PIPELINE_GAPS §7.6",
        "P6-T01",
        "P6-T01 / GC intrinsic support-surface gate",
        FrontendReject,
        true,
        false,
        "GC pin/handle unsupported source shapes must fail at frontend; accepted calls must keep token/root contract"
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
            "PIPELINE_GAPS §3.5",
            "PIPELINE_GAPS §3.6",
            "PIPELINE_GAPS §3.8",
            "PIPELINE_GAPS §3.9",
            "PIPELINE_GAPS §3.10",
            "PIPELINE_GAPS §3.11",
            "PIPELINE_GAPS §3.12",
            "PIPELINE_GAPS §3.13",
            "PIPELINE_GAPS §4.1",
            "PIPELINE_GAPS §4.3",
            "PIPELINE_GAPS §4.4",
            "PIPELINE_GAPS §4.5",
            "PIPELINE_GAPS §5.1",
            "PIPELINE_GAPS §5.3",
            "PIPELINE_GAPS §5.4",
            "PIPELINE_GAPS §7.2",
            "PIPELINE_GAPS §7.6",
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
    fn codegen_gap_inventory_excludes_regression_only_closed_ids() {
        for gap_id in [
            "PIPELINE_GAPS §3.4",
            "PIPELINE_GAPS §3.7",
            "PIPELINE_GAPS §4.2",
            "PIPELINE_GAPS §5.2",
            "PIPELINE_GAPS §5.6",
            "PIPELINE_GAPS §5.7",
            "PIPELINE_GAPS §6.1",
            "PIPELINE_GAPS §6.2",
            "PIPELINE_GAPS §6.3",
            "PIPELINE_GAPS §6.4",
            "PIPELINE_GAPS §6.5",
            "PIPELINE_GAPS §9",
        ] {
            assert!(
                codegen_gap_entry(gap_id).is_none(),
                "{gap_id} should stay out of the active codegen inventory once it only has regression coverage"
            );
        }
    }

    #[test]
    fn codegen_gap_inventory_has_no_blockers_or_stale_legacy_owners() {
        for entry in CODEGEN_GAP_INVENTORY {
            assert!(
                !entry.production_blocker,
                "{} should no longer stay in the active inventory as a production blocker",
                entry.gap_id
            );
            assert!(
                !entry.owner_task.starts_with("CG-T"),
                "{} still points at a stale legacy owner: {}",
                entry.gap_id,
                entry.owner_task
            );
            assert!(
                !entry.suggested_owner.starts_with("CG-T"),
                "{} still points at a stale legacy suggested owner: {}",
                entry.gap_id,
                entry.suggested_owner
            );
        }
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
                            | CodegenGapRoute::EffectLoweredLlvm
                            | CodegenGapRoute::UpstreamMirContract
                            | CodegenGapRoute::FrontendReject
                    ),
                    "upstream contract need must be tied to a concrete route: {entry:?}"
                );
            }
        }
    }

    #[test]
    fn codegen_gap_inventory_marks_p5_t02_pattern_and_closure_gaps_as_closed_guards() {
        let pattern = codegen_gap_entry("PIPELINE_GAPS §3.8")
            .expect("§3.8 guard should remain tracked in inventory");
        assert_eq!(pattern.owner_task, "P5-T02");
        assert_eq!(
            pattern.suggested_owner,
            "P5-T02 / pattern type-test gate and guard"
        );
        assert_eq!(pattern.route, CodegenGapRoute::FrontendReject);
        assert!(!pattern.needs_upstream_contract);
        assert!(!pattern.production_blocker);
        assert_eq!(
            pattern.trigger,
            "when-pattern runtime type test target must stay in ref/string or statically folded value surface"
        );

        let closure = codegen_gap_entry("PIPELINE_GAPS §3.11")
            .expect("§3.11 guard should remain tracked in inventory");
        assert_eq!(closure.owner_task, "P5-T02");
        assert_eq!(
            closure.suggested_owner,
            "P5-T02 / closure env composite transport guard"
        );
        assert_eq!(closure.route, CodegenGapRoute::EffectLoweredLlvm);
        assert!(closure.needs_upstream_contract);
        assert!(!closure.production_blocker);
        assert_eq!(
            closure.trigger,
            "closure env transport must publish shared descriptor-backed env/capture contract"
        );
    }

    #[test]
    fn codegen_gap_inventory_marks_p5_t01_composite_transport_gaps_as_closed_guards() {
        for (gap_id, suggested_owner, route, needs_upstream_contract, trigger) in [
            (
                "PIPELINE_GAPS §4.1",
                "P5-T01 / composite value erasure guard",
                CodegenGapRoute::EffectLoweredLlvm,
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
                CodegenGapRoute::EffectLoweredLlvm,
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
    fn codegen_gap_inventory_marks_p6_t01_runtime_cast_and_gc_gaps_as_closed_guards() {
        let runtime_cast = codegen_gap_entry("PIPELINE_GAPS §3.5")
            .expect("§3.5 guard should remain tracked in inventory");
        assert_eq!(runtime_cast.owner_task, "P6-T01");
        assert_eq!(
            runtime_cast.suggested_owner,
            "P6-T01 / runtime cast contract guard"
        );
        assert_eq!(runtime_cast.route, CodegenGapRoute::EffectLoweredLlvm);
        assert!(runtime_cast.needs_upstream_contract);
        assert!(!runtime_cast.production_blocker);
        assert_eq!(
            runtime_cast.trigger,
            "runtime cast/typecheck metadata must stay within supported runtime-ref or static-folded surface"
        );

        let gc = codegen_gap_entry("PIPELINE_GAPS §7.6")
            .expect("§7.6 guard should remain tracked in inventory");
        assert_eq!(gc.owner_task, "P6-T01");
        assert_eq!(
            gc.suggested_owner,
            "P6-T01 / GC intrinsic support-surface gate"
        );
        assert_eq!(gc.route, CodegenGapRoute::FrontendReject);
        assert!(gc.needs_upstream_contract);
        assert!(!gc.production_blocker);
        assert_eq!(
            gc.trigger,
            "GC pin/handle unsupported source shapes must fail at frontend; accepted calls must keep token/root contract"
        );
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
    fn codegen_gap_inventory_marks_p3_t03_storemember_gap_as_closed_guard() {
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
            assert_eq!(entry.route, CodegenGapRoute::EffectLoweredLlvm);
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
        assert_eq!(entry.route, CodegenGapRoute::EffectLoweredLlvm);
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
    fn codegen_gap_inventory_marks_p6_t02_function_type_cast_gate() {
        let entry = codegen_gap_entry("PIPELINE_GAPS §7.2")
            .expect("§7.2 frontend gate should remain tracked in inventory");
        assert_eq!(entry.owner_task, "P6-T02");
        assert_eq!(
            entry.suggested_owner,
            "P6-T02 / function-type runtime cast frontend gate"
        );
        assert_eq!(entry.route, CodegenGapRoute::FrontendReject);
        assert!(!entry.needs_upstream_contract);
        assert!(!entry.production_blocker);
        assert_eq!(
            entry.trigger,
            "function-type runtime cast must fail at the frontend until a real runtime cast contract exists"
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
            "runtime cast/typecheck metadata",
        ] {
            assert!(
                triggers.contains(needle),
                "inventory trigger list should cover `{needle}`"
            );
        }
    }
}
