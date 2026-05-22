//! LIR fact product shared by backend-neutral consumers.
//!
//! This crate is intentionally data-only: it depends only on base crates for
//! stable identity and compilation context primitives. It does not depend on the
//! `scoopc` facade, MIR/effect stage outputs, the LIR implementation module, or
//! backend ABI types. The P5 LIR stage publishes this product next to the LIR
//! body as the backend-neutral home for callable ABI, dynamic invoke, dispatch,
//! continuation/resume, and LIR opt pipeline contracts.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use scoopc_ids::StableLirCallableKey;
use scoopc_project_model::OptLevel;

pub mod contract;
pub mod dump;
pub mod verify;

pub use contract::*;

/// Complete LIR fact product published by the LIR stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LirFacts {
    pub summary: LirStageSummary,
    pub opt_pipeline: LirOptPipelineFacts,
    pub callables: BTreeMap<StableLirCallableKey, LirCallableFacts>,
    pub step_types: BTreeMap<LirStepSchemaKey, LirStepTypeFacts>,
    pub dynamic_invokes: BTreeMap<LirDynamicInvokeKey, LirDynamicInvokeContract>,
    pub dispatches: BTreeMap<LirDispatchKey, LirDispatchContract>,
    pub resume_packings: BTreeMap<LirResumePackingKey, LirResumePackingFacts>,
    pub continuation_objects: BTreeMap<LirContinuationObjectKey, LirContinuationObjectFacts>,
    pub surface_resume_dispatches:
        BTreeMap<LirContinuationSchemaKey, LirSurfaceResumeDispatchFacts>,
}

/// Grouped LIR facts used to construct a complete product.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LirFactGroups {
    pub callables: BTreeMap<StableLirCallableKey, LirCallableFacts>,
    pub step_types: BTreeMap<LirStepSchemaKey, LirStepTypeFacts>,
    pub dynamic_invokes: BTreeMap<LirDynamicInvokeKey, LirDynamicInvokeContract>,
    pub dispatches: BTreeMap<LirDispatchKey, LirDispatchContract>,
    pub resume_packings: BTreeMap<LirResumePackingKey, LirResumePackingFacts>,
    pub continuation_objects: BTreeMap<LirContinuationObjectKey, LirContinuationObjectFacts>,
    pub surface_resume_dispatches:
        BTreeMap<LirContinuationSchemaKey, LirSurfaceResumeDispatchFacts>,
}

impl LirFacts {
    /// Create an empty fact product for tests and staged construction.
    pub fn new(opt_level: OptLevel) -> Self {
        Self {
            summary: LirStageSummary::new(opt_level),
            opt_pipeline: LirOptPipelineFacts::empty(0),
            callables: BTreeMap::new(),
            step_types: BTreeMap::new(),
            dynamic_invokes: BTreeMap::new(),
            dispatches: BTreeMap::new(),
            resume_packings: BTreeMap::new(),
            continuation_objects: BTreeMap::new(),
            surface_resume_dispatches: BTreeMap::new(),
        }
    }

    /// Build a fact product from already materialized LIR fact groups.
    pub fn from_parts(summary: LirStageSummary, groups: LirFactGroups) -> Self {
        Self::from_parts_with_opt_pipeline(
            summary,
            LirOptPipelineFacts::empty(summary.opt_revision),
            groups,
        )
    }

    /// Build a fact product with explicit LIR opt pipeline metadata.
    pub fn from_parts_with_opt_pipeline(
        summary: LirStageSummary,
        opt_pipeline: LirOptPipelineFacts,
        groups: LirFactGroups,
    ) -> Self {
        Self {
            summary,
            opt_pipeline,
            callables: groups.callables,
            step_types: groups.step_types,
            dynamic_invokes: groups.dynamic_invokes,
            dispatches: groups.dispatches,
            resume_packings: groups.resume_packings,
            continuation_objects: groups.continuation_objects,
            surface_resume_dispatches: groups.surface_resume_dispatches,
        }
    }

    /// Return whether all currently published LIR fact groups are empty.
    pub fn is_empty(&self) -> bool {
        self.callables.is_empty()
            && self.summary.callable_count == 0
            && self.summary.step_type_count == 0
            && self.summary.resume_packing_count == 0
            && self.summary.continuation_object_count == 0
            && self.summary.surface_resume_dispatch_count == 0
            && self.opt_pipeline.passes.is_empty()
            && self.step_types.is_empty()
            && self.dynamic_invokes.is_empty()
            && self.dispatches.is_empty()
            && self.resume_packings.is_empty()
            && self.continuation_objects.is_empty()
            && self.surface_resume_dispatches.is_empty()
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
    pub opt_revision: u64,
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
            opt_revision: 0,
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

    pub fn with_opt_revision(mut self, opt_revision: u64) -> Self {
        self.opt_revision = opt_revision;
        self
    }
}

impl Default for LirStageSummary {
    fn default() -> Self {
        Self::new(OptLevel::O0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify::VerifyError;
    use scoopc_ids::BodyVersionKey;
    use scoopc_types::{TypeId, TypeStore};

    fn ty(raw: u32) -> TypeId {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        match raw {
            1 => builtins.any,
            2 => builtins.unit,
            3 => builtins.string,
            4 => builtins.int,
            _ => builtins.nothing,
        }
    }

    fn body_version(owner: &StableLirCallableKey) -> LirBodyVersionFacts {
        LirBodyVersionFacts {
            key: BodyVersionKey::new(owner, "plain", 0),
            impl_plan: "NoOutward".to_string(),
            needs_reentry: false,
            allowed_effect_terms: Vec::new(),
        }
    }

    fn plain_callable(root_fqn: &str) -> LirCallableFacts {
        let key = StableLirCallableKey::new(format!("lir(instance({root_fqn}))"), root_fqn);
        LirCallableFacts {
            root_fqn: root_fqn.to_string(),
            stable_instance_key: key.as_str().to_string(),
            body_version: body_version(&key),
            resolved_outward_cases: Vec::new(),
            contract: LirCallableContract::Plain(Box::new(LirPlainCallableFacts {
                function_ty: ty(1),
                param_tys: Vec::new(),
                return_ty: ty(2),
                body_slices: Vec::new(),
                call_sites: Vec::new(),
                local_effect_control: None,
            })),
        }
    }

    #[test]
    fn empty_lir_facts_verify_and_dump_group_boundaries() {
        let facts = LirFacts::new(OptLevel::O2);

        assert!(facts.is_empty());
        assert!(facts.verify().is_ok());

        let dump = facts.dump();
        assert!(dump.contains("lir_facts {"));
        assert!(dump.contains("opt_level: O2"));
        assert!(dump.contains("opt_pipeline: revision=0"));
        assert!(dump.contains("callables=0"));
    }

    #[test]
    fn verifier_rejects_opt_revision_mismatch() {
        let facts = LirFacts::from_parts_with_opt_pipeline(
            LirStageSummary::new(OptLevel::O0).with_opt_revision(1),
            LirOptPipelineFacts::empty(2),
            LirFactGroups::default(),
        );

        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::OptRevisionMismatch {
                summary: 1,
                pipeline: 2,
            }
        );
    }

    #[test]
    fn verifier_rejects_summary_callable_count_mismatch() {
        let mut callables = BTreeMap::new();
        callables.insert(
            StableLirCallableKey::new("lir(instance(app.main))", "app.main"),
            plain_callable("app.main"),
        );
        let facts = LirFacts::from_parts(
            LirStageSummary::new(OptLevel::O0),
            LirFactGroups {
                callables,
                ..LirFactGroups::default()
            },
        );

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
        let callable_key = StableLirCallableKey::new("lir(instance(app.main))", "app.main");
        let step_schema = LirStepSchemaKey::new(0);
        let object_id = LirContinuationObjectKey::new(0);
        let mut callables = BTreeMap::new();
        callables.insert(
            callable_key.clone(),
            LirCallableFacts {
                root_fqn: "app.main".to_string(),
                stable_instance_key: callable_key.as_str().to_string(),
                body_version: LirBodyVersionFacts {
                    key: BodyVersionKey::new(&callable_key, "effect_step", 0),
                    impl_plan: "CanonicalFull".to_string(),
                    needs_reentry: true,
                    allowed_effect_terms: Vec::new(),
                },
                resolved_outward_cases: Vec::new(),
                contract: LirCallableContract::EffectStep(Box::new(LirEffectStepCallableFacts {
                    param_tys: Vec::new(),
                    closure_carrier_arg_tys: Vec::new(),
                    step_schema,
                    dynamic_invoke_entry: LirCallableDynamicInvokeEntryFacts {
                        invoke_args_tuple_ty: ty(1),
                        step_schema,
                        entry_state: LirStateKey::new(0),
                        complete_state: LirStateKey::new(1),
                    },
                    control_body: LirControlBodyFacts {
                        step_schema,
                        state_graph: LirStateGraphFacts {
                            entry_state: LirStateKey::new(0),
                            complete_state: LirStateKey::new(1),
                            cleanup_state: None,
                            drop_state: None,
                            states: vec![LirStateKey::new(0), LirStateKey::new(1)],
                        },
                        frame_schema: LirFrameSchemaFacts {
                            slots: Vec::new(),
                            resume_payload_bindings: Vec::new(),
                            completion_payload_bindings: Vec::new(),
                        },
                        boundary_map: LirBoundaryMapFacts {
                            boundaries: Vec::new(),
                        },
                        resume_state_map: LirResumeStateMapFacts {
                            entries: Vec::new(),
                        },
                        source_statement_count: 0,
                        continuation_object: object_id,
                        resume_packings: Vec::new(),
                    },
                })),
            },
        );
        let mut step_types = BTreeMap::new();
        step_types.insert(
            step_schema,
            LirStepTypeFacts {
                step_schema,
                invoke_args_tuple_ty: ty(1),
                complete_ty: ty(2),
                continuation_obj_ty: ty(3),
                cases: Vec::new(),
            },
        );
        let mut continuation_objects = BTreeMap::new();
        continuation_objects.insert(
            object_id,
            LirContinuationObjectFacts {
                object_id,
                owner_body_version: BodyVersionKey::new(&callable_key, "effect_step", 0),
                continuation_obj_ty: ty(3),
                implemented_packings: Vec::new(),
                surface_resumes: Vec::new(),
                methods: Vec::new(),
            },
        );
        let summary = LirStageSummary::new(OptLevel::O2).with_counts(1, 1, 0, 1, 1);
        let facts = LirFacts::from_parts(
            summary,
            LirFactGroups {
                callables,
                step_types,
                continuation_objects,
                surface_resume_dispatches: BTreeMap::from([(
                    LirContinuationSchemaKey::new(0),
                    LirSurfaceResumeDispatchFacts {
                        continuation_schema: LirContinuationSchemaKey::new(0),
                        resume_tuple_ty: ty(4),
                        answer_ty: ty(2),
                        out_step_schema: step_schema,
                        source_kind: LirSurfaceResumeDispatchSourceKind::Unreachable,
                        publication_count: 0,
                        wrapper_projection_count: 0,
                    },
                )]),
                ..LirFactGroups::default()
            },
        );

        assert!(facts.verify().is_ok());
        assert!(facts.dump().contains("callable=app.main kind=EffectStep"));
    }
}
