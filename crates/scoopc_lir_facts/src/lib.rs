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

use scoop_project_model::OptLevel;
use scoopc_ids::StableLirCallableKey;

pub mod contract;
pub mod dump;
pub mod verify;

pub use contract::*;

/// Complete LIR fact product published by the LIR stage.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirFacts {
    pub schema_version: scoopc_types::WireSchemaVersion,
    pub summary: LirStageSummary,
    pub opt_pipeline: LirOptPipelineFacts,
    pub global_init: LirGlobalInitFacts,
    pub physical_layout: LirPhysicalLayoutFacts,
    pub type_context: LirTypeContextFacts,
    pub source_signatures: BTreeMap<String, LirSourceCallableSignatureFacts>,
    pub intrinsic_callables: BTreeMap<String, LirIntrinsicCallableFact>,
    pub source_call_sites: BTreeMap<LirSourceCallSiteKey, LirSourceCallSiteFacts>,
    pub class_ctor_call_sites: BTreeMap<LirClassCtorCallSiteKey, LirClassCtorCallSiteFacts>,
    pub reflection_call_sites: BTreeMap<LirReflectionCallSiteKey, LirReflectionCallSiteFacts>,
    pub class_ctor_inits: BTreeMap<LirClassCtorInitKey, LirClassCtorInitFacts>,
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
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LirFactGroups {
    pub global_init: LirGlobalInitFacts,
    pub physical_layout: LirPhysicalLayoutFacts,
    pub type_context: LirTypeContextFacts,
    pub source_signatures: BTreeMap<String, LirSourceCallableSignatureFacts>,
    pub intrinsic_callables: BTreeMap<String, LirIntrinsicCallableFact>,
    pub source_call_sites: BTreeMap<LirSourceCallSiteKey, LirSourceCallSiteFacts>,
    pub class_ctor_call_sites: BTreeMap<LirClassCtorCallSiteKey, LirClassCtorCallSiteFacts>,
    pub reflection_call_sites: BTreeMap<LirReflectionCallSiteKey, LirReflectionCallSiteFacts>,
    pub class_ctor_inits: BTreeMap<LirClassCtorInitKey, LirClassCtorInitFacts>,
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
            schema_version: scoopc_types::WIRE_SCHEMA_VERSION,
            summary: LirStageSummary::new(opt_level),
            opt_pipeline: LirOptPipelineFacts::empty(0),
            global_init: LirGlobalInitFacts::default(),
            physical_layout: LirPhysicalLayoutFacts::default(),
            type_context: LirTypeContextFacts::default(),
            source_signatures: BTreeMap::new(),
            intrinsic_callables: BTreeMap::new(),
            source_call_sites: BTreeMap::new(),
            class_ctor_call_sites: BTreeMap::new(),
            reflection_call_sites: BTreeMap::new(),
            class_ctor_inits: BTreeMap::new(),
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
            schema_version: scoopc_types::WIRE_SCHEMA_VERSION,
            summary,
            opt_pipeline,
            global_init: groups.global_init,
            physical_layout: groups.physical_layout,
            type_context: groups.type_context,
            source_signatures: groups.source_signatures,
            intrinsic_callables: groups.intrinsic_callables,
            source_call_sites: groups.source_call_sites,
            class_ctor_call_sites: groups.class_ctor_call_sites,
            reflection_call_sites: groups.reflection_call_sites,
            class_ctor_inits: groups.class_ctor_inits,
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
            && self.intrinsic_callables.is_empty()
            && self.source_call_sites.is_empty()
            && self.class_ctor_call_sites.is_empty()
            && self.reflection_call_sites.is_empty()
            && self.class_ctor_inits.is_empty()
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    use scoop_project_model::StableConeKey;
    use scoopc_ids::{BodyVersionKey, SiteId};
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
        let key = StableLirCallableKey::new(
            format!("lir_callable(instance({root_fqn}),body#hfixture)"),
            root_fqn,
        );
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

    fn callable_key(root_fqn: &str) -> StableLirCallableKey {
        StableLirCallableKey::new(
            format!("lir_callable(instance({root_fqn}),body#hfixture)"),
            root_fqn,
        )
    }

    fn source_signature(root_fqn: &str) -> LirSourceCallableSignatureFacts {
        LirSourceCallableSignatureFacts {
            signature_key: format!("sig:{root_fqn}"),
            root_fqn: root_fqn.to_string(),
            param_names: Vec::new(),
            param_tys: Vec::new(),
            return_ty: ty(2),
        }
    }

    fn abi_symbol(root_fqn: &str, callable: Option<StableLirCallableKey>) -> LirAbiSymbolFact {
        LirAbiSymbolFact {
            key: format!("abi:{root_fqn}"),
            symbol: format!("{}_abi", root_fqn.replace('.', "_")),
            callable,
            root_fqn: Some(root_fqn.to_string()),
            role: "extern_callable".to_string(),
        }
    }

    fn call_target_binding(root_fqn: &str, target: StableLirCallableKey) -> LirCallTargetBinding {
        LirCallTargetBinding {
            target_callable_key: target,
            root_fqn: root_fqn.to_string(),
            abi_symbol: format!("{}_abi", root_fqn.replace('.', "_")),
            signature_key: format!("sig:{root_fqn}"),
        }
    }

    fn plain_callable_with_candidate(
        owner_fqn: &str,
        target_root: &str,
        target: StableLirCallableKey,
    ) -> LirCallableFacts {
        let mut callable = plain_callable(owner_fqn);
        let LirCallableContract::Plain(plain) = &mut callable.contract else {
            unreachable!("plain_callable returns a plain contract");
        };
        plain.call_sites.push(LirPlainCallSiteFacts {
            site_id: SiteId::from_raw(1),
            source_slice: LirSourceSliceKey {
                block_id: LirBodyBlockKey::new(0),
                start_statement_index: 0,
                end_statement_index: 1,
                includes_terminator: false,
            },
            statement_index: 0,
            contract: LirCallSiteContract {
                kind: LirCallSiteKind::Direct,
                target_mode: LirCallTargetMode::CandidateSet,
                target_callables: vec![target.clone()],
                target_bindings: vec![call_target_binding(target_root, target)],
                exact_callee: None,
                callee_abi_kind: LirCallableAbiKind::Plain,
                invoke_args_tuple_ty: ty(1),
                callee_step_schema: None,
                resolved_cases: Vec::new(),
                precision: LirEffectPrecision::Precise,
            },
            dynamic_invoke: None,
            dispatch: None,
        });
        callable
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
    fn lir_facts_bincode_round_trip_preserves_schema_and_content() {
        let facts = LirFacts::new(OptLevel::O2);
        let bytes = bincode::serialize(&facts).expect("serialize LIR facts");
        let decoded: LirFacts = bincode::deserialize(&bytes).expect("deserialize LIR facts");

        assert_eq!(decoded.schema_version, scoopc_types::WIRE_SCHEMA_VERSION);
        assert_eq!(decoded, facts);
    }

    #[test]
    fn lir_facts_bincode_round_trip_preserves_control_continuation_contracts() {
        let callable_key =
            StableLirCallableKey::new("lir_callable(instance(app.main),body#hfixture)", "app.main");
        let body_version = BodyVersionKey::new(&callable_key, "effect-step", 0);
        let step_schema = LirStepSchemaKey::new(1);
        let continuation_schema = LirContinuationSchemaKey::new(2);
        let case_tag = LirCaseKey::new(3);
        let resume_packing = LirResumePackingKey::new(4);
        let continuation_object = LirContinuationObjectKey::new(5);
        let entry_state = LirStateKey::new(6);
        let complete_state = LirStateKey::new(7);
        let resume_state = LirStateKey::new(8);
        let boundary = LirBoundaryKey::new(9);
        let frame_slot = LirFrameSlotKey::new(10);
        let dynamic_invoke = LirDynamicInvokeKey {
            owner_callable: callable_key.clone(),
            site_id: scoopc_ids::SiteId::from_raw(11),
        };
        let call_contract = LirCallSiteContract {
            kind: LirCallSiteKind::Closure,
            target_mode: LirCallTargetMode::DynamicFallback,
            target_callables: vec![callable_key.clone()],
            target_bindings: Vec::new(),
            exact_callee: None,
            callee_abi_kind: LirCallableAbiKind::EffectStep,
            invoke_args_tuple_ty: ty(3),
            callee_step_schema: Some(step_schema),
            resolved_cases: vec![case_tag],
            precision: LirEffectPrecision::Precise,
        };
        let control_body = LirControlBodyFacts {
            step_schema,
            state_graph: LirStateGraphFacts {
                entry_state,
                complete_state,
                cleanup_state: None,
                drop_state: None,
                states: vec![entry_state, complete_state, resume_state],
            },
            frame_schema: LirFrameSchemaFacts {
                slots: vec![LirFrameSlotFacts {
                    slot_id: frame_slot,
                    ty: ty(3),
                    kind: "resume_payload".to_string(),
                }],
                resume_payload_bindings: vec![LirResumePayloadBindingFacts {
                    boundary_id: boundary,
                    resume_state,
                    consumer_local: LirLocalKey::new(12),
                    consumer_frame_slot: Some(frame_slot),
                }],
                completion_payload_bindings: Vec::new(),
            },
            boundary_map: LirBoundaryMapFacts {
                boundaries: vec![LirBoundaryFacts {
                    boundary_id: boundary,
                    source_kind: "perform".to_string(),
                    site_id: Some(scoopc_ids::SiteId::from_raw(11)),
                    owner_state: entry_state,
                    resume_state,
                    lowering_kind: Some("dynamic_invoke".to_string()),
                    dynamic_invoke: Some(dynamic_invoke.clone()),
                    dispatch: None,
                }],
            },
            resume_state_map: LirResumeStateMapFacts {
                entries: vec![LirResumeStateFacts {
                    boundary_id: boundary,
                    state_id: resume_state,
                }],
            },
            source_statement_count: 1,
            continuation_object,
            resume_packings: vec![resume_packing],
        };
        let resume_facts = LirContinuationResumeFacts {
            case_tag,
            continuation_schema,
            resume_tuple_ty: ty(3),
            answer_ty: ty(2),
            out_step_schema: step_schema,
            surface_ty: ty(1),
            body: LirContinuationResumeBody::ResumeCapturedState,
        };

        let mut groups = LirFactGroups::default();
        groups.callables.insert(
            callable_key.clone(),
            LirCallableFacts {
                root_fqn: "app.main".to_string(),
                stable_instance_key: callable_key.as_str().to_string(),
                source_kind: LirCallableSourceKind::TopLevel,
                param_names: vec!["message".to_string()],
                param_tys: vec![ty(3)],
                return_ty: ty(2),
                body_version: LirBodyVersionFacts {
                    key: body_version.clone(),
                    impl_plan: "CanonicalFull".to_string(),
                    needs_reentry: true,
                    allowed_effect_terms: vec![ty(1)],
                },
                resolved_outward_cases: vec![case_tag],
                contract: LirCallableContract::EffectStep(Box::new(LirEffectStepCallableFacts {
                    param_tys: vec![ty(3)],
                    closure_carrier_arg_tys: vec![ty(1)],
                    step_schema,
                    dynamic_invoke_entry: LirCallableDynamicInvokeEntryFacts {
                        invoke_args_tuple_ty: ty(3),
                        step_schema,
                        entry_state,
                        complete_state,
                    },
                    control_body,
                })),
            },
        );
        groups.step_types.insert(
            step_schema,
            LirStepTypeFacts {
                step_schema,
                invoke_args_tuple_ty: ty(3),
                complete_ty: ty(2),
                continuation_obj_ty: ty(1),
                cases: vec![LirStepCaseFacts {
                    case_tag,
                    payload_tuple_ty: ty(3),
                    continuation_schema,
                }],
            },
        );
        groups.dynamic_invokes.insert(
            dynamic_invoke.clone(),
            LirDynamicInvokeContract {
                owner_callable: callable_key,
                owner_step_schema: Some(step_schema),
                site_id: dynamic_invoke.site_id,
                source: LirDynamicInvokeSource::Boundary {
                    boundary_id: boundary,
                },
                call: call_contract,
                carrier: LirDynamicInvokeCarrierContract {
                    kind: LirDynamicInvokeCarrierKind::ClosureObject,
                    source_ty: Some(ty(1)),
                    dispatch: None,
                },
                arg_count: 1,
                target_body_versions: vec![body_version.clone()],
            },
        );
        groups.resume_packings.insert(
            resume_packing,
            LirResumePackingFacts {
                interface_id: resume_packing,
                effect_fqn: "app.Log".to_string(),
                effect_type_args: vec![ty(3)],
                return_step_schema: step_schema,
                methods: vec![LirResumeMethodFacts {
                    case_tag,
                    continuation_schema,
                    resume_tuple_ty: ty(3),
                    answer_ty: ty(2),
                    out_step_schema: step_schema,
                    surface_ty: ty(1),
                }],
            },
        );
        groups.continuation_objects.insert(
            continuation_object,
            LirContinuationObjectFacts {
                object_id: continuation_object,
                owner_body_version: body_version,
                continuation_obj_ty: ty(1),
                implemented_packings: vec![resume_packing],
                surface_resumes: vec![resume_facts.clone()],
                methods: vec![LirContinuationMethodFacts {
                    packing_interface_id: resume_packing,
                    resume: resume_facts.clone(),
                }],
            },
        );
        groups.surface_resume_dispatches.insert(
            continuation_schema,
            LirSurfaceResumeDispatchFacts {
                continuation_schema,
                resume_tuple_ty: ty(3),
                answer_ty: ty(2),
                out_step_schema: step_schema,
                source_kind: LirSurfaceResumeDispatchSourceKind::ContinuationObjectMethod,
                publication_count: 1,
                wrapper_projection_count: 1,
            },
        );
        let facts = LirFacts::from_parts(LirStageSummary::new(OptLevel::O2), groups);

        let bytes = bincode::serialize(&facts).expect("serialize populated LIR facts");
        let decoded: LirFacts =
            bincode::deserialize(&bytes).expect("deserialize populated LIR facts");

        assert_eq!(decoded.schema_version, scoopc_types::WIRE_SCHEMA_VERSION);
        assert_eq!(decoded, facts);
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
            StableLirCallableKey::new("lir_callable(instance(app.main),body#hfixture)", "app.main"),
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
        let callable_key =
            StableLirCallableKey::new("lir_callable(instance(app.main),body#hfixture)", "app.main");
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
    fn verifier_rejects_signature_fallback_call_precision() {
        let callable_key =
            StableLirCallableKey::new("lir_callable(instance(app.main),body#hfixture)", "app.main");
        let mut callable = plain_callable("app.main");
        let LirCallableContract::Plain(plain) = &mut callable.contract else {
            panic!("fixture plain callable should have plain contract");
        };
        plain.call_sites.push(LirPlainCallSiteFacts {
            site_id: scoopc_ids::SiteId::from_raw(1),
            source_slice: LirSourceSliceKey {
                block_id: LirBodyBlockKey::new(0),
                start_statement_index: 0,
                end_statement_index: 1,
                includes_terminator: false,
            },
            statement_index: 0,
            contract: LirCallSiteContract {
                kind: LirCallSiteKind::FunValue,
                target_mode: LirCallTargetMode::DynamicFallback,
                target_callables: Vec::new(),
                target_bindings: Vec::new(),
                exact_callee: None,
                callee_abi_kind: LirCallableAbiKind::Plain,
                invoke_args_tuple_ty: ty(2),
                callee_step_schema: None,
                resolved_cases: Vec::new(),
                precision: LirEffectPrecision::SignatureFallback,
            },
            dynamic_invoke: None,
            dispatch: None,
        });
        let facts = LirFacts::from_parts(
            LirStageSummary::new(OptLevel::O0).with_counts(1, 0, 0, 0, 0),
            LirFactGroups {
                callables: BTreeMap::from([(callable_key, callable)]),
                ..LirFactGroups::default()
            },
        );

        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::InvalidExactCalleeBinding {
                callable: "app.main".to_string(),
                reason: "call-site still uses signature-fallback precision",
            }
        );
    }

    #[test]
    fn verifier_rejects_candidate_target_without_published_abi_contract() {
        let owner = callable_key("app.main");
        let target = callable_key("dep.extern_fun");
        let callable = plain_callable_with_candidate("app.main", "dep.extern_fun", target);
        let facts = LirFacts::from_parts(
            LirStageSummary::new(OptLevel::O0).with_counts(1, 0, 0, 0, 0),
            LirFactGroups {
                callables: BTreeMap::from([(owner, callable)]),
                ..LirFactGroups::default()
            },
        );

        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::InvalidExactCalleeBinding {
                callable: "app.main".to_string(),
                reason: "target callable lacks a target-bound source signature or ABI symbol",
            }
        );
    }

    #[test]
    fn verifier_accepts_declaration_only_candidate_target_with_abi_contract() {
        let owner = callable_key("app.main");
        let target = callable_key("dep.extern_fun");
        let callable = plain_callable_with_candidate("app.main", "dep.extern_fun", target.clone());
        let facts = LirFacts::from_parts(
            LirStageSummary::new(OptLevel::O0).with_counts(1, 0, 0, 0, 0),
            LirFactGroups {
                source_signatures: BTreeMap::from([(
                    "dep.extern_fun".to_string(),
                    source_signature("dep.extern_fun"),
                )]),
                physical_layout: LirPhysicalLayoutFacts {
                    abi_symbols: BTreeMap::from([(
                        "abi:dep.extern_fun".to_string(),
                        abi_symbol("dep.extern_fun", Some(target)),
                    )]),
                    ..LirPhysicalLayoutFacts::default()
                },
                callables: BTreeMap::from([(owner, callable)]),
                ..LirFactGroups::default()
            },
        );

        assert!(facts.verify().is_ok());
    }

    #[test]
    fn verifier_rejects_candidate_target_with_unbound_declaration_abi_contract() {
        let owner = callable_key("app.main");
        let target = callable_key("dep.extern_fun");
        let callable = plain_callable_with_candidate("app.main", "dep.extern_fun", target);
        let facts = LirFacts::from_parts(
            LirStageSummary::new(OptLevel::O0).with_counts(1, 0, 0, 0, 0),
            LirFactGroups {
                source_signatures: BTreeMap::from([(
                    "dep.extern_fun".to_string(),
                    source_signature("dep.extern_fun"),
                )]),
                physical_layout: LirPhysicalLayoutFacts {
                    abi_symbols: BTreeMap::from([(
                        "abi:dep.extern_fun".to_string(),
                        abi_symbol("dep.extern_fun", None),
                    )]),
                    ..LirPhysicalLayoutFacts::default()
                },
                callables: BTreeMap::from([(owner, callable)]),
                ..LirFactGroups::default()
            },
        );

        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::InvalidExactCalleeBinding {
                callable: "app.main".to_string(),
                reason: "target callable lacks a target-bound source signature or ABI symbol",
            }
        );
    }

    #[test]
    fn verifier_rejects_vtable_target_without_source_signature_or_abi_symbol() {
        let facts = LirFacts::from_parts(
            LirStageSummary::new(OptLevel::O0),
            LirFactGroups {
                physical_layout: LirPhysicalLayoutFacts {
                    class_vtables: BTreeMap::from([(
                        "app.Class".to_string(),
                        vec![LirClassVtableSlotFacts {
                            slot: 0,
                            name: "run".to_string(),
                            params_len: 0,
                            has_receiver: true,
                            impl_member_fqn: "app.Class.run".to_string(),
                        }],
                    )]),
                    ..LirPhysicalLayoutFacts::default()
                },
                ..LirFactGroups::default()
            },
        );

        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::InvalidAbiSymbol {
                key: "app.Class.run".to_string(),
                reason: "vtable implementation target lacks a published source signature or ABI symbol",
            }
        );
    }

    #[test]
    fn verifier_accepts_vtable_target_with_source_signature_and_abi_symbol() {
        let facts = LirFacts::from_parts(
            LirStageSummary::new(OptLevel::O0),
            LirFactGroups {
                source_signatures: BTreeMap::from([(
                    "app.Class.run".to_string(),
                    source_signature("app.Class.run"),
                )]),
                physical_layout: LirPhysicalLayoutFacts {
                    class_vtables: BTreeMap::from([(
                        "app.Class".to_string(),
                        vec![LirClassVtableSlotFacts {
                            slot: 0,
                            name: "run".to_string(),
                            params_len: 0,
                            has_receiver: true,
                            impl_member_fqn: "app.Class.run".to_string(),
                        }],
                    )]),
                    abi_symbols: BTreeMap::from([(
                        "abi:app.Class.run".to_string(),
                        abi_symbol("app.Class.run", None),
                    )]),
                    ..LirPhysicalLayoutFacts::default()
                },
                ..LirFactGroups::default()
            },
        );

        assert!(facts.verify().is_ok());
    }

    #[test]
    fn verifier_rejects_itable_target_without_source_signature_or_abi_symbol() {
        let facts = LirFacts::from_parts(
            LirStageSummary::new(OptLevel::O0).with_layout_counts(0, 0, 0, 1, 0),
            LirFactGroups {
                physical_layout: LirPhysicalLayoutFacts {
                    class_itables: BTreeMap::from([(
                        "app.Class".to_string(),
                        LirClassItableFacts {
                            class_fqn: "app.Class".to_string(),
                            entries: vec![LirClassItableEntryFacts {
                                interface_fqn: "app.Interface".to_string(),
                                interface_id: 1,
                                interface_type_name: "app.Interface".to_string(),
                                interface_type_id: 2,
                                runtime_match_type_names: Vec::new(),
                                runtime_match_type_ids: Vec::new(),
                                method_impl_fqns: vec!["app.Class.run".to_string()],
                                method_receiver_type_ids: Vec::new(),
                            }],
                        },
                    )]),
                    ..LirPhysicalLayoutFacts::default()
                },
                ..LirFactGroups::default()
            },
        );

        assert_eq!(
            facts.verify().unwrap_err(),
            VerifyError::InvalidAbiSymbol {
                key: "app.Class.run".to_string(),
                reason: "itable implementation target lacks a published source signature or ABI symbol",
            }
        );
    }

    #[test]
    fn verifier_accepts_itable_target_with_source_signature_and_abi_symbol() {
        let facts = LirFacts::from_parts(
            LirStageSummary::new(OptLevel::O0).with_layout_counts(0, 0, 0, 1, 0),
            LirFactGroups {
                source_signatures: BTreeMap::from([(
                    "app.Class.run".to_string(),
                    source_signature("app.Class.run"),
                )]),
                physical_layout: LirPhysicalLayoutFacts {
                    class_itables: BTreeMap::from([(
                        "app.Class".to_string(),
                        LirClassItableFacts {
                            class_fqn: "app.Class".to_string(),
                            entries: vec![LirClassItableEntryFacts {
                                interface_fqn: "app.Interface".to_string(),
                                interface_id: 1,
                                interface_type_name: "app.Interface".to_string(),
                                interface_type_id: 2,
                                runtime_match_type_names: Vec::new(),
                                runtime_match_type_ids: Vec::new(),
                                method_impl_fqns: vec!["app.Class.run".to_string()],
                                method_receiver_type_ids: Vec::new(),
                            }],
                        },
                    )]),
                    abi_symbols: BTreeMap::from([(
                        "abi:app.Class.run".to_string(),
                        abi_symbol("app.Class.run", None),
                    )]),
                    ..LirPhysicalLayoutFacts::default()
                },
                ..LirFactGroups::default()
            },
        );

        assert!(facts.verify().is_ok());
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
