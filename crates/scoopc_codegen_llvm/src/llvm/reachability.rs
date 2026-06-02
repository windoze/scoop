//! LLVM lowering reachability collection over backend-neutral LIR facts.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use scoopc_ids::{StableCanonicalKey, StableLirCallableKey};
use scoopc_lir_facts::{
    LirCallSiteContract, LirCallTargetMode, LirCallableContract, LirCallableFacts, LirDispatchKey,
    LirDynamicInvokeKey, LirFacts, LirGlobalRootKind,
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
    lir_facts: &LirFacts,
) -> Vec<String> {
    let mut collector = ReachabilityCollector::new(lir_facts);
    collector.seed_entry(root_fqn);
    collector.seed_global_init_roots();
    collector.seed_published_lir_callables();
    collector.seed_runtime_required_callables();
    collector.collect()
}

struct ReachabilityCollector<'a> {
    lir_facts: &'a LirFacts,
    callable_roots_by_key: HashMap<&'a StableLirCallableKey, &'a str>,
    queue: VecDeque<String>,
    seen: HashSet<String>,
    reachable: BTreeSet<String>,
}

impl<'a> ReachabilityCollector<'a> {
    fn new(lir_facts: &'a LirFacts) -> Self {
        let mut callable_roots_by_key: HashMap<&'a StableLirCallableKey, &'a str> = lir_facts
            .callables
            .iter()
            .map(|(key, facts)| (key, facts.root_fqn()))
            .collect();
        for symbol in lir_facts.physical_layout.abi_symbols.values() {
            let (Some(callable), Some(root_fqn)) =
                (symbol.callable.as_ref(), symbol.root_fqn.as_deref())
            else {
                continue;
            };
            callable_roots_by_key.entry(callable).or_insert(root_fqn);
        }
        Self {
            lir_facts,
            callable_roots_by_key,
            queue: VecDeque::new(),
            seen: HashSet::new(),
            reachable: BTreeSet::new(),
        }
    }

    fn collect(mut self) -> Vec<String> {
        while let Some(root_fqn) = self.queue.pop_front() {
            if !self.reachable.insert(root_fqn.clone()) {
                continue;
            }
            self.enqueue_callable_edges(&root_fqn);
        }
        self.reachable.into_iter().collect()
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
            .lir_facts
            .callables
            .values()
            .map(|callable| callable.root_fqn().to_string())
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

    fn enqueue_callable_edges(&mut self, root_fqn: &str) {
        let Some(edges) = self.callable_edges(root_fqn) else {
            return;
        };
        for contract in edges.call_contracts {
            self.enqueue_call_contract_targets(&contract);
        }
        for key in edges.dynamic_keys {
            self.enqueue_dynamic_invoke_targets(&key);
        }
        for key in edges.dispatch_keys {
            self.enqueue_dispatch_targets(&key);
        }
    }

    fn callable_edges(&self, root_fqn: &str) -> Option<CallableEdges> {
        let callable = self.callable_by_root(root_fqn)?;
        let mut edges = CallableEdges::default();
        match &callable.contract {
            LirCallableContract::Plain(plain) => {
                for call_site in &plain.call_sites {
                    edges.call_contracts.push(call_site.contract.clone());
                    if let Some(key) = call_site.dynamic_invoke.as_ref() {
                        edges.dynamic_keys.push(key.clone());
                    }
                    if let Some(key) = call_site.dispatch.as_ref() {
                        edges.dispatch_keys.push(key.clone());
                    }
                }
                if let Some(control) = plain.local_effect_control.as_ref() {
                    for boundary in &control.boundary_map.boundaries {
                        if let Some(key) = boundary.dynamic_invoke.as_ref() {
                            edges.dynamic_keys.push(key.clone());
                        }
                        if let Some(key) = boundary.dispatch.as_ref() {
                            edges.dispatch_keys.push(key.clone());
                        }
                    }
                }
            }
            LirCallableContract::EffectStep(effect) => {
                for boundary in &effect.control_body.boundary_map.boundaries {
                    if let Some(key) = boundary.dynamic_invoke.as_ref() {
                        edges.dynamic_keys.push(key.clone());
                    }
                    if let Some(key) = boundary.dispatch.as_ref() {
                        edges.dispatch_keys.push(key.clone());
                    }
                }
            }
        }

        edges.dynamic_keys.extend(
            self.lir_facts
                .dynamic_invokes
                .iter()
                .filter(|(_, dynamic)| {
                    self.root_for_callable_key(&dynamic.owner_callable)
                        .as_deref()
                        == Some(root_fqn)
                })
                .map(|(key, _)| key.clone()),
        );
        edges.dispatch_keys.extend(
            self.lir_facts
                .dispatches
                .iter()
                .filter(|(_, dispatch)| {
                    self.root_for_callable_key(&dispatch.owner_callable)
                        .as_deref()
                        == Some(root_fqn)
                })
                .map(|(key, _)| key.clone()),
        );
        Some(edges)
    }

    fn enqueue_dynamic_invoke_targets(&mut self, key: &LirDynamicInvokeKey) {
        let Some(dynamic) = self.lir_facts.dynamic_invokes.get(key) else {
            return;
        };
        let contract = dynamic.call.clone();
        let dispatch = dynamic.carrier.dispatch.clone();
        self.enqueue_call_contract_targets(&contract);
        if let Some(dispatch) = dispatch.as_ref() {
            self.enqueue_dispatch_targets(dispatch);
        }
    }

    fn enqueue_dispatch_targets(&mut self, key: &LirDispatchKey) {
        let Some(dispatch) = self.lir_facts.dispatches.get(key) else {
            return;
        };
        let targets = dispatch.candidate_targets.clone();
        for target in &targets {
            self.enqueue_required_callable_key(target);
        }
    }

    fn enqueue_call_contract_targets(&mut self, contract: &LirCallSiteContract) {
        if let Some(exact) = contract.exact_callee.as_ref() {
            self.enqueue_root(&exact.root_fqn);
            return;
        }
        for target in &contract.target_callables {
            match contract.target_mode {
                LirCallTargetMode::KnownInstance
                | LirCallTargetMode::CandidateSet
                | LirCallTargetMode::DynamicFallback => self.enqueue_required_callable_key(target),
            }
        }
    }

    fn enqueue_required_callable_key(&mut self, key: &StableLirCallableKey) {
        if let Some(root) = self.root_for_callable_key(key) {
            self.enqueue_root(&root);
        } else {
            panic!(
                "LIR reachability verifier accepted required callable key `{}` without a published root",
                key.canonical_text()
            );
        }
    }

    fn enqueue_root(&mut self, root_fqn: &str) {
        if self.seen.insert(root_fqn.to_string()) {
            self.queue.push_back(root_fqn.to_string());
        }
    }

    fn callable_by_root(&self, root_fqn: &str) -> Option<&'a LirCallableFacts> {
        self.lir_facts
            .callables
            .values()
            .find(|callable| callable.root_fqn() == root_fqn)
    }

    fn root_for_callable_key(&self, key: &StableLirCallableKey) -> Option<String> {
        self.callable_roots_by_key
            .get(key)
            .map(|root| (*root).to_string())
    }
}

#[derive(Default)]
struct CallableEdges {
    call_contracts: Vec<LirCallSiteContract>,
    dynamic_keys: Vec<LirDynamicInvokeKey>,
    dispatch_keys: Vec<LirDispatchKey>,
}

#[cfg(all(test, not(feature = "standalone-codegen-crate")))]
mod tests {
    use super::*;
    use scoop_project_model::{OptLevel, StableConeKey};
    use scoopc_ids::{BodyVersionKey, SiteId, StableLirCallableKey};
    use scoopc_lir_facts::{
        LirBodyVersionFacts, LirBoundaryMapFacts, LirCallSiteContract, LirCallSiteKind,
        LirCallTargetMode, LirCallableAbiKind, LirCallableContract,
        LirCallableDynamicInvokeEntryFacts, LirCallableFacts, LirCallableSourceKind,
        LirContinuationObjectKey, LirControlBodyFacts, LirDispatchContract, LirDispatchKey,
        LirDynamicInvokeCarrierContract, LirDynamicInvokeCarrierKind, LirDynamicInvokeContract,
        LirDynamicInvokeKey, LirDynamicInvokeSource, LirEffectPrecision,
        LirEffectStepCallableFacts, LirFactGroups, LirFrameSchemaFacts, LirGlobalInitFacts,
        LirGlobalRootFacts, LirGlobalRootKey, LirGlobalRootKind, LirPlainCallSiteFacts,
        LirPlainCallableFacts, LirResumeStateMapFacts, LirSourceCallableSignatureFacts,
        LirSourceSliceKey, LirStageSummary, LirStateGraphFacts, LirStateKey, LirStepSchemaKey,
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

    fn call_contract(targets: Vec<StableLirCallableKey>) -> LirCallSiteContract {
        LirCallSiteContract {
            kind: LirCallSiteKind::Direct,
            target_mode: LirCallTargetMode::CandidateSet,
            target_callables: targets,
            exact_callee: None,
            callee_abi_kind: LirCallableAbiKind::Plain,
            invoke_args_tuple_ty: ty(1),
            callee_step_schema: None,
            resolved_cases: Vec::new(),
            precision: LirEffectPrecision::Precise,
        }
    }

    fn plain_callable(root_fqn: &str, targets: Vec<StableLirCallableKey>) -> LirCallableFacts {
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
                    continuation_object: LirContinuationObjectKey::new(0),
                    resume_packings: Vec::new(),
                },
            })),
        }
    }

    fn facts_with_callables(callables: Vec<LirCallableFacts>) -> LirFacts {
        let mut map = std::collections::BTreeMap::new();
        for callable in callables {
            map.insert(callable_key(callable.root_fqn()), callable);
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
            plain_callable("app.main", vec![callable_key("app.helper")]),
            plain_callable("app.helper", vec![callable_key("app.leaf")]),
            plain_callable("app.leaf", Vec::new()),
        ]);

        assert_eq!(
            collect_reachable_top_level_funs("app.main", &facts),
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
            collect_reachable_top_level_funs("app.main", &facts),
            vec!["app.init_helper".to_string(), "app.main".to_string()]
        );
    }

    #[test]
    fn reachability_uses_lir_dynamic_and_dispatch_targets() {
        let owner = callable_key("app.main");
        let target = callable_key("app.impl");
        let dispatch_key = LirDispatchKey {
            owner_callable: owner.clone(),
            site_id: SiteId::from_raw(7),
        };
        let dynamic_key = LirDynamicInvokeKey {
            owner_callable: owner.clone(),
            site_id: SiteId::from_raw(7),
        };
        let mut facts = facts_with_callables(vec![
            plain_callable("app.main", Vec::new()),
            plain_callable("app.impl", Vec::new()),
        ]);
        facts.dispatches.insert(
            dispatch_key.clone(),
            LirDispatchContract {
                owner_callable: owner.clone(),
                site_id: SiteId::from_raw(7),
                kind: LirCallSiteKind::Interface,
                owner_fqn: "app.IFace".to_string(),
                member_name: "run".to_string(),
                member_fqn: "app.IFace.run".to_string(),
                receiver_ty: ty(1),
                explicit_arg_count: 0,
                method_slot: 0,
                interface_id: Some(0),
                candidate_targets: vec![target.clone()],
            },
        );
        facts.dynamic_invokes.insert(
            dynamic_key,
            LirDynamicInvokeContract {
                owner_callable: owner,
                owner_step_schema: None,
                site_id: SiteId::from_raw(7),
                source: LirDynamicInvokeSource::PlainCallSite {
                    source_slice: LirSourceSliceKey {
                        block_id: scoopc_lir_facts::LirBodyBlockKey::new(0),
                        start_statement_index: 0,
                        end_statement_index: 1,
                        includes_terminator: false,
                    },
                    statement_index: 0,
                },
                call: LirCallSiteContract {
                    kind: LirCallSiteKind::Interface,
                    target_mode: LirCallTargetMode::CandidateSet,
                    target_callables: vec![target],
                    exact_callee: None,
                    callee_abi_kind: LirCallableAbiKind::Plain,
                    invoke_args_tuple_ty: ty(1),
                    callee_step_schema: None,
                    resolved_cases: Vec::new(),
                    precision: LirEffectPrecision::Precise,
                },
                carrier: LirDynamicInvokeCarrierContract {
                    kind: LirDynamicInvokeCarrierKind::InterfaceReceiver,
                    source_ty: None,
                    dispatch: Some(dispatch_key),
                },
                arg_count: 0,
                target_body_versions: Vec::new(),
            },
        );

        assert_eq!(
            collect_reachable_top_level_funs("app.main", &facts),
            vec!["app.impl".to_string(), "app.main".to_string()]
        );
    }

    #[test]
    #[should_panic(expected = "without a published root")]
    fn reachability_rejects_unpublished_candidate_set_targets() {
        let facts = facts_with_callables(vec![plain_callable(
            "app.main",
            vec![callable_key("scoop.core.Bool.toString")],
        )]);

        let _ = collect_reachable_top_level_funs("app.main", &facts);
    }

    #[test]
    fn reachability_includes_declaration_only_candidate_set_targets() {
        let target = callable_key("scoop.core.Bool.toString");
        let mut facts =
            facts_with_callables(vec![plain_callable("app.main", vec![target.clone()])]);
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
                callable: Some(target),
                root_fqn: Some("scoop.core.Bool.toString".to_string()),
                role: "extern_callable".to_string(),
            },
        );

        assert_eq!(
            collect_reachable_top_level_funs("app.main", &facts),
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
            collect_reachable_top_level_funs("app.main", &facts),
            vec![
                "app.main".to_string(),
                "app.main$continuation_resume".to_string(),
            ]
        );
    }
}
