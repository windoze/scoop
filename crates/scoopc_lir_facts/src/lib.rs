//! LIR fact product shared by backend-neutral consumers.
//!
//! This crate is intentionally data-only: it depends only on base crates for
//! stable identity and compilation context primitives. It does not depend on the
//! `scoopc` facade, MIR/effect stage outputs, the LIR implementation module, or
//! backend ABI types. The P5 LIR stage publishes this product next to the LIR
//! body so later backend-neutral queries have a stable home.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use scoopc_ids::StableLirCallableKey;
use scoopc_project_model::OptLevel;

pub mod dump;
pub mod verify;

/// Complete LIR fact product published by the LIR stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LirFacts {
    pub summary: LirStageSummary,
    pub callables: BTreeMap<StableLirCallableKey, LirCallableSummary>,
}

impl LirFacts {
    /// Create an empty fact product for tests and staged construction.
    pub fn new(opt_level: OptLevel) -> Self {
        Self {
            summary: LirStageSummary::new(opt_level),
            callables: BTreeMap::new(),
        }
    }

    /// Build a fact product from already materialized LIR fact groups.
    pub fn from_parts(
        summary: LirStageSummary,
        callables: BTreeMap<StableLirCallableKey, LirCallableSummary>,
    ) -> Self {
        Self { summary, callables }
    }

    /// Return whether all currently published LIR fact groups are empty.
    pub fn is_empty(&self) -> bool {
        self.callables.is_empty()
            && self.summary.callable_count == 0
            && self.summary.step_type_count == 0
            && self.summary.resume_packing_count == 0
            && self.summary.continuation_object_count == 0
            && self.summary.surface_resume_dispatch_count == 0
    }

    /// Verify structural invariants before handing facts to later stages.
    pub fn verify(&self) -> verify::Result<()> {
        verify::verify_lir_facts(self)
    }

    /// Render a stable textual summary of the LIR fact groups.
    pub fn dump(&self) -> String {
        dump::dump_lir_facts(self)
    }
}

/// Stage-level LIR summary fixed by the P5 output shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LirStageSummary {
    pub opt_level: OptLevel,
    pub callable_count: usize,
    pub step_type_count: usize,
    pub resume_packing_count: usize,
    pub continuation_object_count: usize,
    pub surface_resume_dispatch_count: usize,
}

impl LirStageSummary {
    pub fn new(opt_level: OptLevel) -> Self {
        Self {
            opt_level,
            callable_count: 0,
            step_type_count: 0,
            resume_packing_count: 0,
            continuation_object_count: 0,
            surface_resume_dispatch_count: 0,
        }
    }

    pub fn with_counts(
        mut self,
        callable_count: usize,
        step_type_count: usize,
        resume_packing_count: usize,
        continuation_object_count: usize,
        surface_resume_dispatch_count: usize,
    ) -> Self {
        self.callable_count = callable_count;
        self.step_type_count = step_type_count;
        self.resume_packing_count = resume_packing_count;
        self.continuation_object_count = continuation_object_count;
        self.surface_resume_dispatch_count = surface_resume_dispatch_count;
        self
    }
}

impl Default for LirStageSummary {
    fn default() -> Self {
        Self::new(OptLevel::O0)
    }
}

/// Backend-neutral callable kind published by LIR facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LirCallableKind {
    Plain,
    EffectStep,
}

/// Minimal callable inventory entry published by the P5 output shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LirCallableSummary {
    root_fqn: String,
    kind: LirCallableKind,
    has_control_body: bool,
}

impl LirCallableSummary {
    pub fn new(root_fqn: impl Into<String>, kind: LirCallableKind, has_control_body: bool) -> Self {
        Self {
            root_fqn: root_fqn.into(),
            kind,
            has_control_body,
        }
    }

    pub fn root_fqn(&self) -> &str {
        &self.root_fqn
    }

    pub fn kind(&self) -> LirCallableKind {
        self.kind
    }

    pub fn has_control_body(&self) -> bool {
        self.has_control_body
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify::VerifyError;

    #[test]
    fn empty_lir_facts_verify_and_dump_group_boundaries() {
        let facts = LirFacts::new(OptLevel::O2);

        assert!(facts.is_empty());
        assert!(facts.verify().is_ok());

        let dump = facts.dump();
        assert!(dump.contains("lir_facts {"));
        assert!(dump.contains("opt_level: O2"));
        assert!(dump.contains("callables=0"));
    }

    #[test]
    fn verifier_rejects_summary_callable_count_mismatch() {
        let mut callables = BTreeMap::new();
        callables.insert(
            StableLirCallableKey::new("lir(instance(app.main))", "app.main"),
            LirCallableSummary::new("app.main", LirCallableKind::Plain, false),
        );
        let facts = LirFacts::from_parts(LirStageSummary::new(OptLevel::O0), callables);

        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::CallableCountMismatch {
                expected: 0,
                actual: 1,
            }
        );
    }

    #[test]
    fn verifier_accepts_callable_inventory_summary() {
        let mut callables = BTreeMap::new();
        callables.insert(
            StableLirCallableKey::new("lir(instance(app.main))", "app.main"),
            LirCallableSummary::new("app.main", LirCallableKind::EffectStep, true),
        );
        let summary = LirStageSummary::new(OptLevel::O2).with_counts(1, 1, 0, 1, 1);
        let facts = LirFacts::from_parts(summary, callables);

        assert!(facts.verify().is_ok());
        assert!(facts.dump().contains("callable=app.main kind=EffectStep"));
    }
}
