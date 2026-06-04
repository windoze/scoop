//! LLVM lowering reachability collection over backend-neutral LIR facts.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use scoopc_ids::LirCallableId;
use scoopc_lir::effect_lowered::LateLoweredProgram;
use scoopc_lir_facts::{
    LirCallSiteContract, LirCallTargetMode, LirCallableContract, LirCallableFacts, LirCallableRef,
    LirDispatchContract, LirDynamicInvokeContract, LirFacts, LirGlobalRootKind,
};

/// Runtime helpers whose source callables may need legacy declarations until
/// LLVM body emission is fully moved to LIR contracts.
const RUNTIME_REQUIRED_CALLABLES: &[&str] = &[];

/// Collect source-level callable FQNs needed by LLVM legacy declaration checks.
///
/// This intentionally walks only LIR/LIR-facts contracts. HIR body scans, raw
/// MIR scans, and backend-local dispatch target refinement are owned by earlier
/// stages and must not reappear in backend reachability.
pub(super) fn collect_reachable_top_level_funs(
    root_fqn: &str,
    lir: &LateLoweredProgram,
    lir_facts: &LirFacts,
) -> Result<Vec<String>, String> {
    let mut collector = ReachabilityCollector::new(lir, lir_facts);
    collector.seed_entry(root_fqn);
    collector.seed_global_init_roots();
    collector.seed_published_lir_callables();
    collector.seed_runtime_required_callables();
    collector.collect()
}

struct ReachabilityCollector<'a> {
    lir: &'a LateLoweredProgram,
    lir_facts: &'a LirFacts,
    callable_roots_by_id: HashMap<LirCallableId, &'a str>,
    callable_ids_by_root: HashMap<&'a str, LirCallableId>,
    queue: VecDeque<String>,
    seen: HashSet<String>,
    reachable: BTreeSet<String>,
}

impl<'a> ReachabilityCollector<'a> {
    fn new(lir: &'a LateLoweredProgram, lir_facts: &'a LirFacts) -> Self {
        let mut callable_roots_by_id: HashMap<LirCallableId, &'a str> = HashMap::new();
        let mut callable_ids_by_root: HashMap<&'a str, LirCallableId> = HashMap::new();
        for (index, callable) in lir.callables().iter().enumerate() {
            let Some(id) = LirCallableId::from_index(index) else {
                continue;
            };
            callable_roots_by_id
                .entry(id)
                .or_insert(callable.root_fqn());
            callable_ids_by_root
                .entry(callable.root_fqn())
                .or_insert(id);
        }
        Self {
            lir,
            lir_facts,
            callable_roots_by_id,
            callable_ids_by_root,
            queue: VecDeque::new(),
            seen: HashSet::new(),
            reachable: BTreeSet::new(),
        }
    }

    fn collect(mut self) -> Result<Vec<String>, String> {
        while let Some(root_fqn) = self.queue.pop_front() {
            if !self.reachable.insert(root_fqn.clone()) {
                continue;
            }
            self.enqueue_callable_edges(&root_fqn)?;
        }
        Ok(self.reachable.into_iter().collect())
    }

    fn seed_entry(&mut self, root_fqn: &str) {
        self.enqueue_root(root_fqn);
    }

    fn seed_global_init_roots(&mut self) {
        let mut roots = Vec::new();
        for root in self.lir_facts.global_init.roots.values() {
            match root.kind {
                LirGlobalRootKind::TopLevelImmutableVal
                | LirGlobalRootKind::TopLevelMutableVar
                | LirGlobalRootKind::ObjectSingleton => roots.push(root.root.as_str().to_string()),
                LirGlobalRootKind::ExternGlobal => {}
            }
            for dependency in &root.dependencies {
                roots.push(dependency.target.as_str().to_string());
            }
        }
        for routine in self.lir_facts.global_init.cone_init_routines.values() {
            for root in &routine.roots {
                roots.push(root.as_str().to_string());
            }
        }
        for root in roots {
            self.enqueue_root(&root);
        }
    }

    fn seed_published_lir_callables(&mut self) {
        let roots = self
            .callable_ids_by_root
            .keys()
            .map(|root| (*root).to_string())
            .collect::<Vec<_>>();
        for root in roots {
            self.enqueue_root(&root);
        }
    }

    fn seed_runtime_required_callables(&mut self) {
        for root in RUNTIME_REQUIRED_CALLABLES {
            self.enqueue_root(root);
        }
    }

    fn enqueue_callable_edges(&mut self, root_fqn: &str) -> Result<(), String> {
        let Some(edges) = self.callable_edges(root_fqn)? else {
            return Ok(());
        };
        for contract in edges.call_contracts {
            self.enqueue_call_contract_targets(&contract)?;
        }
        for dynamic in edges.dynamic_invokes {
            self.enqueue_dynamic_invoke_targets(&dynamic)?;
        }
        for dispatch in edges.dispatches {
            self.enqueue_dispatch_targets(&dispatch)?;
        }
        Ok(())
    }

    fn callable_edges(&self, root_fqn: &str) -> Result<Option<CallableEdges>, String> {
        let Some(callable) = self.callable_by_root(root_fqn) else {
            if self.root_has_published_declaration(root_fqn) {
                return Ok(None);
            }
            return Err(format!(
                "reachable root `{root_fqn}` is missing LIR callable and target-bound declaration facts"
            ));
        };
        let mut edges = CallableEdges::default();
        match &callable.contract {
            LirCallableContract::Plain(plain) => {
                for call_site in &plain.call_sites {
                    edges.call_contracts.push(call_site.contract.clone());
                    if let Some(dynamic) = call_site.dynamic_invoke.as_ref() {
                        edges.dynamic_invokes.push(dynamic.clone());
                    }
                    if let Some(dispatch) = call_site.dispatch.as_ref() {
                        edges.dispatches.push(dispatch.clone());
                    }
                }
                if let Some(control) = plain.local_effect_control.as_ref() {
                    for boundary in &control.boundary_map.boundaries {
                        if let Some(dynamic) = boundary.dynamic_invoke.as_ref() {
                            edges.dynamic_invokes.push(dynamic.clone());
                        }
                        if let Some(dispatch) = boundary.dispatch.as_ref() {
                            edges.dispatches.push(dispatch.clone());
                        }
                    }
                    for site in &control.source_statement_call_sites {
                        if let Some(dynamic) = site.dynamic_invoke.as_ref() {
                            edges.dynamic_invokes.push(dynamic.clone());
                        }
                        if let Some(dispatch) = site.dispatch.as_ref() {
                            edges.dispatches.push(dispatch.clone());
                        }
                    }
                }
            }
            LirCallableContract::EffectStep(effect) => {
                for boundary in &effect.control_body.boundary_map.boundaries {
                    if let Some(dynamic) = boundary.dynamic_invoke.as_ref() {
                        edges.dynamic_invokes.push(dynamic.clone());
                    }
                    if let Some(dispatch) = boundary.dispatch.as_ref() {
                        edges.dispatches.push(dispatch.clone());
                    }
                }
                for site in &effect.control_body.source_statement_call_sites {
                    if let Some(dynamic) = site.dynamic_invoke.as_ref() {
                        edges.dynamic_invokes.push(dynamic.clone());
                    }
                    if let Some(dispatch) = site.dispatch.as_ref() {
                        edges.dispatches.push(dispatch.clone());
                    }
                }
            }
        }
        Ok(Some(edges))
    }

    fn enqueue_dynamic_invoke_targets(
        &mut self,
        dynamic: &LirDynamicInvokeContract,
    ) -> Result<(), String> {
        let contract = dynamic.call.clone();
        let dispatch = dynamic.carrier.dispatch.clone();
        self.enqueue_call_contract_targets(&contract)?;
        if let Some(dispatch) = dispatch.as_ref() {
            self.enqueue_dispatch_targets(dispatch)?;
        }
        Ok(())
    }

    fn enqueue_dispatch_targets(&mut self, dispatch: &LirDispatchContract) -> Result<(), String> {
        let targets = dispatch.candidate_targets.clone();
        for target in &targets {
            self.enqueue_required_callable_ref(*target)?;
        }
        Ok(())
    }

    fn enqueue_call_contract_targets(
        &mut self,
        contract: &LirCallSiteContract,
    ) -> Result<(), String> {
        if let Some(exact) = contract.exact_callee.as_ref() {
            self.enqueue_root(&exact.root_fqn);
            return Ok(());
        }
        for target in &contract.target_callables {
            match contract.target_mode {
                LirCallTargetMode::KnownInstance
                | LirCallTargetMode::CandidateSet
                | LirCallTargetMode::DynamicFallback => {
                    self.enqueue_required_callable_ref(*target)?
                }
            }
        }
        Ok(())
    }

    fn enqueue_required_callable_ref(&mut self, target: LirCallableRef) -> Result<(), String> {
        let root = self.required_root_for_callable_ref(target)?;
        self.enqueue_root(&root);
        Ok(())
    }

    fn enqueue_root(&mut self, root_fqn: &str) {
        if self.seen.insert(root_fqn.to_string()) {
            self.queue.push_back(root_fqn.to_string());
        }
    }

    fn callable_by_root(&self, root_fqn: &str) -> Option<&'a LirCallableFacts> {
        self.lir.callable(root_fqn)?.published_callable_facts()
    }

    fn root_for_callable_ref(&self, target: LirCallableRef) -> Option<String> {
        match target {
            LirCallableRef::Local(id) => self
                .callable_roots_by_id
                .get(&id)
                .map(|root| (*root).to_string()),
            LirCallableRef::ExternalHash(_) => self
                .lir_facts
                .physical_layout
                .abi_symbols
                .values()
                .find_map(|symbol| {
                    (symbol.callable == Some(target))
                        .then(|| symbol.root_fqn.clone())
                        .flatten()
                }),
        }
    }

    fn root_has_published_declaration(&self, root_fqn: &str) -> bool {
        self.lir_facts
            .global_init
            .roots
            .values()
            .any(|root| root.root.as_str() == root_fqn)
            || (self.lir.source_signature(root_fqn).is_some()
                && self
                    .lir_facts
                    .physical_layout
                    .abi_symbols
                    .values()
                    .any(|symbol| {
                        symbol.root_fqn.as_deref() == Some(root_fqn)
                            && symbol.callable.is_some()
                            && matches!(
                                symbol.role.as_str(),
                                "callable_export" | "native_callable" | "extern_callable"
                            )
                    }))
    }

    fn required_root_for_callable_ref(&self, target: LirCallableRef) -> Result<String, String> {
        self.root_for_callable_ref(target).ok_or_else(|| {
            format!(
                "call target `{}` is not published in LIR callable or target-bound ABI facts",
                target.display_text()
            )
        })
    }
}

#[derive(Default)]
struct CallableEdges {
    call_contracts: Vec<LirCallSiteContract>,
    dynamic_invokes: Vec<LirDynamicInvokeContract>,
    dispatches: Vec<LirDispatchContract>,
}

#[cfg(all(test, not(feature = "standalone-codegen-crate")))]
mod tests {
    use super::*;
    use scoop_project_model::{OptLevel, StableConeKey};
    use scoopc_ids::{
        BodyVersionKey, LirCallableHash, LirCallableId, SiteId, StableLirCallableKey,
    };
    use scoopc_lir_facts::{
        LirBodyVersionFacts, LirBoundaryMapFacts, LirCallSiteContract, LirCallSiteKind,
        LirCallTargetMode, LirCallableAbiKind, LirCallableContract,
        LirCallableDynamicInvokeEntryFacts, LirCallableFacts, LirCallableRef,
        LirCallableSourceKind, LirContinuationObjectKey, LirControlBodyFacts, LirDispatchContract,
        LirDynamicInvokeCarrierContract, LirDynamicInvokeCarrierKind, LirDynamicInvokeContract,
        LirDynamicInvokeSource, LirEffectPrecision, LirEffectStepCallableFacts, LirFactGroups,
        LirFrameSchemaFacts, LirGlobalInitFacts, LirGlobalRootFacts, LirGlobalRootKey,
        LirGlobalRootKind, LirPlainCallSiteFacts, LirPlainCallableFacts, LirResumeStateMapFacts,
        LirSourceCallableSignatureFacts, LirSourceSliceKey, LirStageSummary, LirStateGraphFacts,
        LirStateKey, LirStepSchemaKey,
    };
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

    fn callable_key(root_fqn: &str) -> StableLirCallableKey {
        StableLirCallableKey::new(
            format!("lir_callable(instance({root_fqn}),body#hfixture)"),
            root_fqn,
        )
    }

    fn body_version(owner: &StableLirCallableKey) -> LirBodyVersionFacts {
        LirBodyVersionFacts {
            key: BodyVersionKey::new(owner, "plain", 0),
            impl_plan: "NoOutward".to_string(),
            needs_reentry: false,
            allowed_effect_terms: Vec::new(),
        }
    }

    fn call_contract(targets: Vec<LirCallableRef>) -> LirCallSiteContract {
        LirCallSiteContract {
            kind: LirCallSiteKind::Direct,
            target_mode: LirCallTargetMode::CandidateSet,
            target_callables: targets,
            target_bindings: Vec::new(),
            exact_callee: None,
            callee_abi_kind: LirCallableAbiKind::Plain,
            invoke_args_tuple_ty: ty(1),
            callee_step_schema: None,
            resolved_cases: Vec::new(),
            precision: LirEffectPrecision::Precise,
        }
    }

    fn plain_callable(root_fqn: &str, targets: Vec<LirCallableRef>) -> LirCallableFacts {
        let key = callable_key(root_fqn);
        let call_sites = if targets.is_empty() {
            Vec::new()
        } else {
            vec![LirPlainCallSiteFacts {
                site_id: SiteId::from_raw(0),
                source_slice: LirSourceSliceKey {
                    block_id: scoopc_lir_facts::LirBodyBlockKey::new(0),
                    start_statement_index: 0,
                    end_statement_index: 1,
                    includes_terminator: false,
                },
                statement_index: 0,
                contract: call_contract(targets),
                dynamic_invoke: None,
                dispatch: None,
            }]
        };
        LirCallableFacts {
            root_fqn: root_fqn.to_string(),
            stable_instance_key: key.as_str().to_string(),
            source_kind: LirCallableSourceKind::TopLevel,
            param_names: Vec::new(),
            param_tys: Vec::new(),
            return_ty: ty(2),
            body_version: body_version(&key),
            resolved_outward_cases: Vec::new(),
            source_call_sites: Vec::new(),
            class_ctor_call_sites: Vec::new(),
            reflection_call_sites: Vec::new(),
            contract: LirCallableContract::Plain(Box::new(LirPlainCallableFacts {
                function_ty: ty(1),
                param_names: Vec::new(),
                param_tys: Vec::new(),
                return_ty: ty(2),
                body_slices: Vec::new(),
                call_sites,
                local_effect_control: None,
            })),
        }
    }

    fn effect_step_callable(root_fqn: &str) -> LirCallableFacts {
        let key = callable_key(root_fqn);
        let step_schema = LirStepSchemaKey::new(0);
        LirCallableFacts {
            root_fqn: root_fqn.to_string(),
            stable_instance_key: key.as_str().to_string(),
            source_kind: LirCallableSourceKind::MemberOrSynthetic,
            param_names: Vec::new(),
            param_tys: Vec::new(),
            return_ty: ty(2),
            body_version: body_version(&key),
            resolved_outward_cases: Vec::new(),
            source_call_sites: Vec::new(),
            class_ctor_call_sites: Vec::new(),
            reflection_call_sites: Vec::new(),
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
                    source_statement_call_sites: Vec::new(),
                    source_statement_count: 0,
                    continuation_object: LirContinuationObjectKey::new(0),
                    resume_packings: Vec::new(),
                },
            })),
        }
    }

    fn facts_with_callables(callables: Vec<LirCallableFacts>) -> LirFacts {
        let mut map = std::collections::BTreeMap::new();
        for (index, callable) in callables.into_iter().enumerate() {
            map.insert(
                LirCallableId::from_index(index).expect("test callable id fits"),
                callable,
            );
        }
        LirFacts::from_parts(
            LirStageSummary::new(OptLevel::O0).with_counts(map.len(), 0, 0, 0, 0),
            LirFactGroups {
                callables: map,
                ..LirFactGroups::default()
            },
        )
    }

    #[test]
    fn reachability_uses_lir_callable_edges() {
        let facts = facts_with_callables(vec![
            plain_callable(
                "app.main",
                vec![LirCallableRef::local(LirCallableId::from_raw(1))],
            ),
            plain_callable(
                "app.helper",
                vec![LirCallableRef::local(LirCallableId::from_raw(2))],
            ),
            plain_callable("app.leaf", Vec::new()),
        ]);

        assert_eq!(
            collect_reachable_top_level_funs("app.main", &facts).unwrap(),
            vec![
                "app.helper".to_string(),
                "app.leaf".to_string(),
                "app.main".to_string(),
            ]
        );
    }

    #[test]
    fn reachability_seeds_global_init_roots() {
        let mut facts = facts_with_callables(vec![plain_callable("app.init_helper", Vec::new())]);
        facts.global_init = LirGlobalInitFacts::default();
        facts.global_init.roots.insert(
            LirGlobalRootKey::new("app.init_helper"),
            LirGlobalRootFacts {
                root: LirGlobalRootKey::new("app.init_helper"),
                kind: LirGlobalRootKind::TopLevelImmutableVal,
                cone: StableConeKey::new("app", "0.0.0"),
                source_cone_order: 0,
                ty: None,
                storage: None,
                has_initializer: true,
                dependencies: Vec::new(),
                source_path: None,
                extern_global: None,
                initializer_body: Some(scoopc_lir_facts::LirInitializerBodyFacts {
                    root: LirGlobalRootKey::new("app.init_helper"),
                    kind: scoopc_lir_facts::LirInitializerBodyKind::TopLevelImmutableVal,
                    source_path: "app.scoop".to_string(),
                    source_span_start: 0,
                    source_span_end: 1,
                    body_item_count: 1,
                }),
            },
        );

        assert_eq!(
            collect_reachable_top_level_funs("app.main", &facts).unwrap(),
            vec!["app.init_helper".to_string(), "app.main".to_string()]
        );
    }

    #[test]
    fn reachability_uses_lir_dynamic_and_dispatch_targets() {
        let mut facts = facts_with_callables(vec![
            plain_callable("app.main", Vec::new()),
            plain_callable("app.impl", Vec::new()),
        ]);
        let target = LirCallableRef::local(LirCallableId::from_raw(1));
        let source_slice = LirSourceSliceKey {
            block_id: scoopc_lir_facts::LirBodyBlockKey::new(0),
            start_statement_index: 0,
            end_statement_index: 1,
            includes_terminator: false,
        };
        let call = LirCallSiteContract {
            kind: LirCallSiteKind::Interface,
            target_mode: LirCallTargetMode::CandidateSet,
            target_callables: vec![target],
            target_bindings: Vec::new(),
            exact_callee: None,
            callee_abi_kind: LirCallableAbiKind::Plain,
            invoke_args_tuple_ty: ty(1),
            callee_step_schema: None,
            resolved_cases: Vec::new(),
            precision: LirEffectPrecision::Precise,
        };
        let dispatch = LirDispatchContract {
            owner_callable: LirCallableId::from_raw(0),
            site_id: SiteId::from_raw(7),
            kind: LirCallSiteKind::Interface,
            owner_fqn: "app.IFace".to_string(),
            member_name: "run".to_string(),
            member_fqn: "app.IFace.run".to_string(),
            receiver_ty: ty(1),
            explicit_arg_count: 0,
            method_slot: 0,
            interface_id: Some(0),
            candidate_targets: vec![target],
        };
        let dynamic = LirDynamicInvokeContract {
            owner_callable: LirCallableId::from_raw(0),
            owner_step_schema: None,
            site_id: SiteId::from_raw(7),
            source: LirDynamicInvokeSource::PlainCallSite {
                source_slice,
                statement_index: 0,
            },
            call: call.clone(),
            carrier: LirDynamicInvokeCarrierContract {
                kind: LirDynamicInvokeCarrierKind::InterfaceReceiver,
                source_ty: None,
                dispatch: Some(dispatch.clone()),
            },
            arg_count: 0,
            target_body_versions: Vec::new(),
        };
        let main = facts
            .callables
            .get_mut(&LirCallableId::from_raw(0))
            .expect("main callable facts exist");
        let LirCallableContract::Plain(plain) = &mut main.contract else {
            panic!("main should be plain");
        };
        plain.call_sites.push(LirPlainCallSiteFacts {
            site_id: SiteId::from_raw(7),
            source_slice,
            statement_index: 0,
            contract: call,
            dynamic_invoke: Some(dynamic),
            dispatch: Some(dispatch),
        });

        assert_eq!(
            collect_reachable_top_level_funs("app.main", &facts).unwrap(),
            vec!["app.impl".to_string(), "app.main".to_string()]
        );
    }

    #[test]
    fn reachability_rejects_unpublished_candidate_set_targets() {
        let target = callable_key("scoop.core.Bool.toString");
        let facts = facts_with_callables(vec![plain_callable(
            "app.main",
            vec![LirCallableRef::external_hash(
                LirCallableHash::from_stable_key(&target),
            )],
        )]);

        assert!(
            collect_reachable_top_level_funs("app.main", &facts)
                .unwrap_err()
                .contains("is not published")
        );
    }

    #[test]
    fn reachability_includes_declaration_only_candidate_set_targets() {
        let target = callable_key("scoop.core.Bool.toString");
        let target_ref = LirCallableRef::external_hash(LirCallableHash::from_stable_key(&target));
        let mut facts = facts_with_callables(vec![plain_callable("app.main", vec![target_ref])]);
        facts.source_signatures.insert(
            "scoop.core.Bool.toString".to_string(),
            LirSourceCallableSignatureFacts {
                signature_key: "sig:scoop.core.Bool.toString".to_string(),
                root_fqn: "scoop.core.Bool.toString".to_string(),
                param_names: Vec::new(),
                param_tys: Vec::new(),
                return_ty: ty(2),
            },
        );
        facts.physical_layout.abi_symbols.insert(
            "abi:scoop.core.Bool.toString".to_string(),
            scoopc_lir_facts::LirAbiSymbolFact {
                key: "abi:scoop.core.Bool.toString".to_string(),
                symbol: "scoop_core_Bool_toString".to_string(),
                callable: Some(target_ref),
                root_fqn: Some("scoop.core.Bool.toString".to_string()),
                role: "extern_callable".to_string(),
            },
        );

        assert_eq!(
            collect_reachable_top_level_funs("app.main", &facts).unwrap(),
            vec![
                "app.main".to_string(),
                "scoop.core.Bool.toString".to_string()
            ]
        );
    }

    #[test]
    fn reachability_seeds_published_continuation_resume_callable() {
        let facts = facts_with_callables(vec![
            plain_callable("app.main", Vec::new()),
            effect_step_callable("app.main$continuation_resume"),
        ]);

        assert_eq!(
            collect_reachable_top_level_funs("app.main", &facts).unwrap(),
            vec![
                "app.main".to_string(),
                "app.main$continuation_resume".to_string(),
            ]
        );
    }
}
