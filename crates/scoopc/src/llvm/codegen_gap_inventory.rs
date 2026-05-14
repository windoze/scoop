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
