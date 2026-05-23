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
    pub global_init: LirGlobalInitFacts,
    pub physical_layout: LirPhysicalLayoutFacts,
    pub type_context: LirTypeContextFacts,
    pub source_signatures: BTreeMap<String, LirSourceCallableSignatureFacts>,
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
    pub global_init: LirGlobalInitFacts,
    pub physical_layout: LirPhysicalLayoutFacts,
    pub type_context: LirTypeContextFacts,
    pub source_signatures: BTreeMap<String, LirSourceCallableSignatureFacts>,
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
            global_init: LirGlobalInitFacts::default(),
            physical_layout: LirPhysicalLayoutFacts::default(),
            type_context: LirTypeContextFacts::default(),
            source_signatures: BTreeMap::new(),
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
            global_init: groups.global_init,
            physical_layout: groups.physical_layout,
            type_context: groups.type_context,
            source_signatures: groups.source_signatures,
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
            && self.source_signatures.is_empty()
            && self.global_init.is_empty()
            && self.physical_layout.is_empty()
            && self.summary.callable_count == 0
            && self.summary.global_root_count == 0
            && self.summary.object_once_count == 0
            && self.summary.top_level_eager_init_count == 0
            && self.summary.cone_init_routine_count == 0
            && self.summary.layout_class_count == 0
            && self.summary.layout_enum_count == 0
            && self.summary.layout_interface_count == 0
            && self.summary.layout_class_itable_count == 0
            && self.summary.layout_callable_symbol_count == 0
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
    pub global_root_count: usize,
    pub object_once_count: usize,
    pub top_level_eager_init_count: usize,
    pub cone_init_routine_count: usize,
    pub layout_class_count: usize,
    pub layout_enum_count: usize,
    pub layout_interface_count: usize,
    pub layout_class_itable_count: usize,
    pub layout_callable_symbol_count: usize,
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
            global_root_count: 0,
            object_once_count: 0,
            top_level_eager_init_count: 0,
            cone_init_routine_count: 0,
            layout_class_count: 0,
            layout_enum_count: 0,
            layout_interface_count: 0,
            layout_class_itable_count: 0,
            layout_callable_symbol_count: 0,
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

    pub fn with_global_counts(
        mut self,
        global_root_count: usize,
        object_once_count: usize,
        top_level_eager_init_count: usize,
        cone_init_routine_count: usize,
    ) -> Self {
        self.global_root_count = global_root_count;
        self.object_once_count = object_once_count;
        self.top_level_eager_init_count = top_level_eager_init_count;
        self.cone_init_routine_count = cone_init_routine_count;
        self
    }

    pub fn with_layout_counts(
        mut self,
        class_count: usize,
        enum_count: usize,
        interface_count: usize,
        class_itable_count: usize,
        callable_symbol_count: usize,
    ) -> Self {
        self.layout_class_count = class_count;
        self.layout_enum_count = enum_count;
        self.layout_interface_count = interface_count;
        self.layout_class_itable_count = class_itable_count;
        self.layout_callable_symbol_count = callable_symbol_count;
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
    use scoopc_project_model::StableConeKey;
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
            source_kind: LirCallableSourceKind::TopLevel,
            param_names: Vec::new(),
            param_tys: Vec::new(),
            return_ty: ty(2),
            body_version: body_version(&key),
            resolved_outward_cases: Vec::new(),
            contract: LirCallableContract::Plain(Box::new(LirPlainCallableFacts {
                function_ty: ty(1),
                param_names: Vec::new(),
                param_tys: Vec::new(),
                return_ty: ty(2),
                body_slices: Vec::new(),
                call_sites: Vec::new(),
                local_effect_control: None,
            })),
        }
    }

    fn global_root_key(fqn: &str) -> LirGlobalRootKey {
        LirGlobalRootKey::new(fqn)
    }

    fn global_root(
        fqn: &str,
        kind: LirGlobalRootKind,
        storage: Option<LirGlobalStoragePolicy>,
        dependencies: Vec<LirGlobalRootDependency>,
    ) -> LirGlobalRootFacts {
        let initializer_body = match kind {
            LirGlobalRootKind::TopLevelImmutableVal => Some(initializer_body(
                fqn,
                LirInitializerBodyKind::TopLevelImmutableVal,
                1,
            )),
            LirGlobalRootKind::TopLevelMutableVar => Some(initializer_body(
                fqn,
                LirInitializerBodyKind::TopLevelMutableVar,
                1,
            )),
            LirGlobalRootKind::ObjectSingleton => Some(initializer_body(
                fqn,
                LirInitializerBodyKind::ObjectSingleton,
                1,
            )),
            LirGlobalRootKind::ExternGlobal => None,
        };
        LirGlobalRootFacts {
            root: global_root_key(fqn),
            kind,
            cone: StableConeKey::new("fixture", "0.0.0"),
            source_cone_order: 0,
            ty: Some(ty(4)),
            storage,
            has_initializer: true,
            dependencies,
            source_path: Some("fixture.scoop".to_string()),
            extern_global: None,
            initializer_body,
        }
    }

    fn initializer_body(
        fqn: &str,
        kind: LirInitializerBodyKind,
        body_item_count: usize,
    ) -> LirInitializerBodyFacts {
        LirInitializerBodyFacts {
            root: global_root_key(fqn),
            kind,
            source_path: "fixture.scoop".to_string(),
            source_span_start: 0,
            source_span_end: 1,
            body_item_count,
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
                source_kind: LirCallableSourceKind::TopLevel,
                param_names: Vec::new(),
                param_tys: Vec::new(),
                return_ty: ty(2),
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

    #[test]
    fn verifier_and_dump_publish_global_init_contracts() {
        let base = global_root(
            "app.Base",
            LirGlobalRootKind::TopLevelImmutableVal,
            None,
            Vec::new(),
        );
        let counter = global_root(
            "app.Counter",
            LirGlobalRootKind::TopLevelMutableVar,
            Some(LirGlobalStoragePolicy::Global),
            vec![LirGlobalRootDependency {
                target: global_root_key("app.Base"),
                kind: LirGlobalDependencyKind::TopLevelValue,
            }],
        );
        let registry = global_root(
            "app.Registry",
            LirGlobalRootKind::ObjectSingleton,
            None,
            Vec::new(),
        );
        let mut global_init = LirGlobalInitFacts::default();
        global_init.roots.insert(base.root.clone(), base);
        global_init.roots.insert(counter.root.clone(), counter);
        global_init.roots.insert(registry.root.clone(), registry);
        global_init.top_level_eager_inits.insert(
            global_root_key("app.Base"),
            LirTopLevelEagerInitFacts {
                root: global_root_key("app.Base"),
                storage: None,
                has_initializer: true,
            },
        );
        global_init.top_level_eager_inits.insert(
            global_root_key("app.Counter"),
            LirTopLevelEagerInitFacts {
                root: global_root_key("app.Counter"),
                storage: Some(LirGlobalStoragePolicy::Global),
                has_initializer: true,
            },
        );
        global_init.object_once.insert(
            global_root_key("app.Registry"),
            LirObjectOnceFacts {
                root: global_root_key("app.Registry"),
                has_initializer: true,
            },
        );
        global_init.cone_init_routines.insert(
            LirConeInitRoutineKey::new(0),
            LirConeInitRoutineFacts {
                routine: LirConeInitRoutineKey::new(0),
                cone: StableConeKey::new("fixture", "0.0.0"),
                source_cone_order: 0,
                roots: vec![global_root_key("app.Base"), global_root_key("app.Counter")],
            },
        );
        global_init.final_entry_order.routines = vec![LirConeInitRoutineKey::new(0)];

        let summary = LirStageSummary::new(OptLevel::O0).with_global_counts(3, 1, 2, 1);
        let facts = LirFacts::from_parts(
            summary,
            LirFactGroups {
                global_init,
                ..LirFactGroups::default()
            },
        );

        assert!(facts.verify().is_ok());
        let dump = facts.dump();
        assert!(dump.contains("global_init: roots=3 object_once=1 top_level_eager_inits=2"));
        assert!(dump.contains("root=app.Counter kind=top_level_mutable_var"));
        assert!(dump.contains("depends_on=app.Base kind=top_level_value"));
        assert!(dump.contains("final_entry_init_order=[r0]"));
    }

    #[test]
    fn verifier_rejects_missing_global_dependency() {
        let mut root = global_root(
            "app.Counter",
            LirGlobalRootKind::TopLevelMutableVar,
            Some(LirGlobalStoragePolicy::Global),
            vec![LirGlobalRootDependency {
                target: global_root_key("app.Missing"),
                kind: LirGlobalDependencyKind::TopLevelValue,
            }],
        );
        root.has_initializer = true;
        let mut global_init = LirGlobalInitFacts::default();
        global_init.roots.insert(root.root.clone(), root);
        global_init.top_level_eager_inits.insert(
            global_root_key("app.Counter"),
            LirTopLevelEagerInitFacts {
                root: global_root_key("app.Counter"),
                storage: Some(LirGlobalStoragePolicy::Global),
                has_initializer: true,
            },
        );
        let facts = LirFacts::from_parts(
            LirStageSummary::new(OptLevel::O0).with_global_counts(1, 0, 1, 0),
            LirFactGroups {
                global_init,
                ..LirFactGroups::default()
            },
        );

        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::MissingGlobalRootDependency {
                root: "app.Counter".to_string(),
                dependency: "app.Missing".to_string(),
            }
        );
    }

    #[test]
    fn verifier_rejects_top_level_eager_storage_drift() {
        let counter = global_root(
            "app.Counter",
            LirGlobalRootKind::TopLevelMutableVar,
            Some(LirGlobalStoragePolicy::Global),
            Vec::new(),
        );
        let mut global_init = LirGlobalInitFacts::default();
        global_init.roots.insert(counter.root.clone(), counter);
        global_init.top_level_eager_inits.insert(
            global_root_key("app.Counter"),
            LirTopLevelEagerInitFacts {
                root: global_root_key("app.Counter"),
                storage: Some(LirGlobalStoragePolicy::ThreadLocal),
                has_initializer: true,
            },
        );
        let facts = LirFacts::from_parts(
            LirStageSummary::new(OptLevel::O0).with_global_counts(1, 0, 1, 0),
            LirFactGroups {
                global_init,
                ..LirFactGroups::default()
            },
        );

        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::MismatchedGlobalStoragePolicy {
                root: "app.Counter".to_string(),
                contract: "top-level eager init",
                root_storage: "global".to_string(),
                contract_storage: "thread_local".to_string(),
            }
        );
    }

    #[test]
    fn verifier_rejects_eager_root_missing_from_cone_routine() {
        let base = global_root(
            "app.Base",
            LirGlobalRootKind::TopLevelImmutableVal,
            None,
            Vec::new(),
        );
        let mut global_init = LirGlobalInitFacts::default();
        global_init.roots.insert(base.root.clone(), base);
        global_init.top_level_eager_inits.insert(
            global_root_key("app.Base"),
            LirTopLevelEagerInitFacts {
                root: global_root_key("app.Base"),
                storage: None,
                has_initializer: true,
            },
        );
        let facts = LirFacts::from_parts(
            LirStageSummary::new(OptLevel::O0).with_global_counts(1, 0, 1, 0),
            LirFactGroups {
                global_init,
                ..LirFactGroups::default()
            },
        );

        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::MissingConeInitRoutineForRoot {
                root: "app.Base".to_string(),
            }
        );
    }

    #[test]
    fn verifier_rejects_eager_dependency_order_drift() {
        let base = global_root(
            "app.Base",
            LirGlobalRootKind::TopLevelImmutableVal,
            None,
            Vec::new(),
        );
        let counter = global_root(
            "app.Counter",
            LirGlobalRootKind::TopLevelMutableVar,
            Some(LirGlobalStoragePolicy::Global),
            vec![LirGlobalRootDependency {
                target: global_root_key("app.Base"),
                kind: LirGlobalDependencyKind::TopLevelValue,
            }],
        );
        let mut global_init = LirGlobalInitFacts::default();
        global_init.roots.insert(base.root.clone(), base);
        global_init.roots.insert(counter.root.clone(), counter);
        global_init.top_level_eager_inits.insert(
            global_root_key("app.Base"),
            LirTopLevelEagerInitFacts {
                root: global_root_key("app.Base"),
                storage: None,
                has_initializer: true,
            },
        );
        global_init.top_level_eager_inits.insert(
            global_root_key("app.Counter"),
            LirTopLevelEagerInitFacts {
                root: global_root_key("app.Counter"),
                storage: Some(LirGlobalStoragePolicy::Global),
                has_initializer: true,
            },
        );
        global_init.cone_init_routines.insert(
            LirConeInitRoutineKey::new(0),
            LirConeInitRoutineFacts {
                routine: LirConeInitRoutineKey::new(0),
                cone: StableConeKey::new("fixture", "0.0.0"),
                source_cone_order: 0,
                roots: vec![global_root_key("app.Counter"), global_root_key("app.Base")],
            },
        );
        global_init.final_entry_order.routines = vec![LirConeInitRoutineKey::new(0)];
        let facts = LirFacts::from_parts(
            LirStageSummary::new(OptLevel::O0).with_global_counts(2, 0, 2, 1),
            LirFactGroups {
                global_init,
                ..LirFactGroups::default()
            },
        );

        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::InvalidConeInitDependencyOrder {
                root: "app.Counter".to_string(),
                dependency: "app.Base".to_string(),
            }
        );
    }

    #[test]
    fn verifier_rejects_final_entry_source_cone_order_drift() {
        let mut dep = global_root(
            "dep.Root",
            LirGlobalRootKind::TopLevelImmutableVal,
            None,
            Vec::new(),
        );
        dep.cone = StableConeKey::new("dep", "0.0.0");
        dep.source_cone_order = 0;
        let mut app = global_root(
            "app.Root",
            LirGlobalRootKind::TopLevelImmutableVal,
            None,
            Vec::new(),
        );
        app.cone = StableConeKey::new("app", "0.0.0");
        app.source_cone_order = 1;

        let mut global_init = LirGlobalInitFacts::default();
        global_init.roots.insert(dep.root.clone(), dep);
        global_init.roots.insert(app.root.clone(), app);
        for root in [global_root_key("dep.Root"), global_root_key("app.Root")] {
            global_init.top_level_eager_inits.insert(
                root.clone(),
                LirTopLevelEagerInitFacts {
                    root,
                    storage: None,
                    has_initializer: true,
                },
            );
        }
        global_init.cone_init_routines.insert(
            LirConeInitRoutineKey::new(0),
            LirConeInitRoutineFacts {
                routine: LirConeInitRoutineKey::new(0),
                cone: StableConeKey::new("app", "0.0.0"),
                source_cone_order: 1,
                roots: vec![global_root_key("app.Root")],
            },
        );
        global_init.cone_init_routines.insert(
            LirConeInitRoutineKey::new(1),
            LirConeInitRoutineFacts {
                routine: LirConeInitRoutineKey::new(1),
                cone: StableConeKey::new("dep", "0.0.0"),
                source_cone_order: 0,
                roots: vec![global_root_key("dep.Root")],
            },
        );
        global_init.final_entry_order.routines =
            vec![LirConeInitRoutineKey::new(0), LirConeInitRoutineKey::new(1)];
        let facts = LirFacts::from_parts(
            LirStageSummary::new(OptLevel::O0).with_global_counts(2, 0, 2, 2),
            LirFactGroups {
                global_init,
                ..LirFactGroups::default()
            },
        );

        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::InvalidConeInitRoutineSourceOrder {
                routine: 1,
                previous_routine: 0,
            }
        );
    }
}
