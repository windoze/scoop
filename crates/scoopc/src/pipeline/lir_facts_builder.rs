//! Production adapter from `LateLoweredProgram` to the data-only LIR facts crate.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use scoopc_ids::{
    AbiMangler, BodyVersionKey, StableCanonicalKey, StableHashScope, StableLirCallableKey,
    StableSymbolKey, canonical_list, canonical_record, stable_hash128_hex,
};
use scoopc_lir_facts::*;
use scoopc_mir_facts::MirFacts;
use scoopc_mir_facts::roots::{
    MirGlobalStorageKind, MirInitializerDependencyKind, MirInitializerRootKind, MirRootDetail,
    MirRootFact,
};

use scoopc_lir::effect_facts::{
    CallSiteEffectFacts, CallSiteKind, CallSiteTarget, CallTargetMode, CallableAbiKind,
    ConcreteOpKey, EffectPrecision, ImplPlan, MaterializedEffectFacts, SiteEffectFacts,
};
use scoopc_lir::effect_lowered::EffectLoweringError;
use scoopc_lir::effect_lowered::ir::{
    BoundarySiteKind, LateLoweredBoundaryLowering, LateLoweredBoundaryMap,
    LateLoweredBoundarySource, LateLoweredCallSiteMaterializedKind,
    LateLoweredCallSiteMaterializedMetadata, LateLoweredCallable,
    LateLoweredContinuationResumeBody, LateLoweredDynamicInvokeEntry,
    LateLoweredEffectStepCallable, LateLoweredFrameSchema, LateLoweredPlainBodySlice,
    LateLoweredPlainCallSite, LateLoweredPlainCallable, LateLoweredProgram,
    LateLoweredResumeStateMap, LateLoweredStateGraph, LateLoweredStateSlice,
    LateLoweredSurfaceResumeDispatchSourceKind,
};
use scoopc_lir::mir::{FunDecl, InstanceKey, Item, MaterializedMir, SiteId};
use scoopc_lir::opt::OptLevel;
use scoopc_lir::stable_id::{NoTypeParamResolver, canonical_effect_row_text, canonical_type_text};
use scoopc_lir::ty::{TypeId, TypeKind, TypeStore, ValueTypeKind};

struct LirFactsBuildContext<'a> {
    lir: &'a LateLoweredProgram,
    mir_facts: &'a MirFacts,
    materialized: &'a MaterializedMir,
    effect_facts: &'a MaterializedEffectFacts,
    callable_keys_by_stable_instance: HashMap<String, StableLirCallableKey>,
    body_versions_by_key: HashMap<StableLirCallableKey, BodyVersionKey>,
}

#[derive(Debug, Clone)]
struct MaterializedCallableSignature {
    source_kind: LirCallableSourceKind,
    param_names: Vec<String>,
    param_tys: Vec<TypeId>,
    return_ty: TypeId,
    closure_carrier_arg_tys: Vec<TypeId>,
}

/// Build the LIR fact product from the authoritative post-opt LIR body.
pub(crate) fn attach_lir_identity(
    lir: LateLoweredProgram,
    types: &TypeStore,
) -> Result<LateLoweredProgram, EffectLoweringError> {
    let mut identities = HashMap::new();
    for callable in lir.callables() {
        let callable_key = stable_lir_callable_key(&lir, types, callable)?;
        let body_version_key = derived_body_version_key(callable, &callable_key);
        identities.insert(
            callable.body_version_key().clone(),
            (callable_key, body_version_key),
        );
    }
    Ok(lir.with_lir_callable_identities(identities))
}

/// Build the LIR fact product from the authoritative post-opt LIR body.
pub(crate) fn build_lir_facts(
    lir: &LateLoweredProgram,
    mir_facts: &MirFacts,
    materialized: &MaterializedMir,
    effect_facts: &MaterializedEffectFacts,
    opt_level: OptLevel,
    opt_pipeline: LirOptPipelineFacts,
) -> Result<LirFacts, EffectLoweringError> {
    let callable_keys_by_stable_instance = callable_key_index(lir)?;
    let body_versions_by_key = body_version_index(lir)?;
    let ctx = LirFactsBuildContext {
        lir,
        mir_facts,
        materialized,
        effect_facts,
        callable_keys_by_stable_instance,
        body_versions_by_key,
    };

    let mut dynamic_invokes = BTreeMap::new();
    let mut dispatches = BTreeMap::new();
    let callables = build_callable_facts(&ctx, &mut dynamic_invokes, &mut dispatches)?;
    let source_signatures = build_source_signature_facts(ctx.lir, ctx.mir_facts)?;
    let groups = LirFactGroups {
        global_init: build_global_init_facts(&ctx)?,
        physical_layout: build_physical_layout_facts(
            &ctx,
            &callables,
            &dynamic_invokes,
            &source_signatures,
        )?,
        type_context: build_type_context_facts(ctx.materialized, ctx.effect_facts),
        source_signatures,
        class_ctor_inits: build_class_ctor_init_facts(ctx.lir),
        callables,
        step_types: build_step_type_facts(lir),
        dynamic_invokes,
        dispatches,
        resume_packings: build_resume_packing_facts(lir),
        continuation_objects: build_continuation_object_facts(&ctx)?,
        surface_resume_dispatches: build_surface_resume_dispatch_facts(lir),
    };
    let summary = LirStageSummary::new(opt_level)
        .with_opt_revision(opt_pipeline.revision)
        .with_counts(
            groups.callables.len(),
            groups.step_types.len(),
            groups.resume_packings.len(),
            groups.continuation_objects.len(),
            groups.surface_resume_dispatches.len(),
        )
        .with_global_counts(
            groups.global_init.roots.len(),
            groups.global_init.object_once.len(),
            groups.global_init.top_level_eager_inits.len(),
            groups.global_init.cone_init_routines.len(),
        )
        .with_layout_counts(
            groups.physical_layout.classes.len(),
            groups.physical_layout.enums.len(),
            groups.physical_layout.interfaces.len(),
            groups.physical_layout.class_itables.len(),
            groups.physical_layout.callable_symbols.len(),
        );
    let facts = LirFacts::from_parts_with_opt_pipeline(summary, opt_pipeline, groups);
    facts
        .verify()
        .map_err(|error| EffectLoweringError::InvalidLirFactsContract {
            detail: error.to_string(),
        })?;
    Ok(facts)
}

fn callable_key_index(
    lir: &LateLoweredProgram,
) -> Result<HashMap<String, StableLirCallableKey>, EffectLoweringError> {
    let mut out = HashMap::new();
    for callable in lir.callables() {
        let key = published_callable_key(callable)?.clone();
        let stable_instance = callable.stable_instance_key().canonical_text();
        if let Some(existing) = out.insert(stable_instance.clone(), key.clone())
            && existing != key
        {
            return Err(EffectLoweringError::InvalidLirFactsContract {
                detail: format!(
                    "stable instance `{stable_instance}` maps to multiple LIR callable keys"
                ),
            });
        }
    }
    Ok(out)
}

fn body_version_index(
    lir: &LateLoweredProgram,
) -> Result<HashMap<StableLirCallableKey, BodyVersionKey>, EffectLoweringError> {
    let mut out = HashMap::new();
    for callable in lir.callables() {
        let key = published_callable_key(callable)?.clone();
        let body_version = published_body_version_key(callable)?.clone();
        out.insert(key, body_version);
    }
    Ok(out)
}

fn stable_lir_callable_key(
    lir: &LateLoweredProgram,
    types: &TypeStore,
    callable: &LateLoweredCallable,
) -> Result<StableLirCallableKey, EffectLoweringError> {
    let stable_instance_key = callable.stable_instance_key();
    let body_canonical = stable_lir_body_version_text(lir, types, callable)?;
    let body_hash = stable_hash128_hex(StableHashScope::AbiV0, &body_canonical);
    Ok(StableLirCallableKey::new(
        canonical_record(
            "lir_callable",
            [
                stable_instance_key.canonical_text(),
                format!("body#h{body_hash}"),
            ],
        ),
        stable_instance_key.readable_path(),
    ))
}

fn stable_lir_body_version_text(
    lir: &LateLoweredProgram,
    types: &TypeStore,
    callable: &LateLoweredCallable,
) -> Result<String, EffectLoweringError> {
    let version_key = callable.body_version_key();
    Ok(canonical_record(
        "lir_body_version",
        [
            callable.stable_instance_key().canonical_text(),
            canonical_effect_row_fragment(
                types,
                version_key.allowed_row(),
                "LIR callable allowed row",
            )?,
            impl_plan_key_text(lir, types, callable, "LIR callable impl plan")?,
            version_key.needs_reentry().to_string(),
        ],
    ))
}

fn impl_plan_key_text(
    lir: &LateLoweredProgram,
    types: &TypeStore,
    callable: &LateLoweredCallable,
    context: &str,
) -> Result<String, EffectLoweringError> {
    match callable.impl_plan() {
        ImplPlan::NoOutward => Ok(canonical_record("impl_plan", ["no_outward".to_string()])),
        ImplPlan::CanonicalFull => Ok(canonical_record(
            "impl_plan",
            ["canonical_full".to_string()],
        )),
        ImplPlan::SingleCase(case_tag) => {
            let step_schema = callable.body_step_schema().ok_or_else(|| {
                invalid_lir_facts_error(format!(
                    "{context} 的 single-case impl_plan 缺少 owner step schema"
                ))
            })?;
            let step_type = lir.step_type(step_schema).ok_or_else(|| {
                invalid_lir_facts_error(format!(
                    "{context} 缺少 step schema {} 对应的 late-lowered step type",
                    step_schema.as_u32()
                ))
            })?;
            let case = step_type.case(case_tag).ok_or_else(|| {
                invalid_lir_facts_error(format!(
                    "{context} 的 single-case impl_plan 缺少 case {}",
                    case_tag.as_u32()
                ))
            })?;
            Ok(canonical_record(
                "impl_plan",
                [canonical_record(
                    "single_case",
                    [
                        concrete_op_key_text(
                            types,
                            case.concrete_op_key(),
                            &format!("{context} concrete op"),
                        )?,
                        canonical_type_fragment(
                            types,
                            case.payload_tuple_ty(),
                            &format!("{context} payload tuple"),
                        )?,
                        continuation_contract_shallow_text(
                            lir,
                            types,
                            case.continuation_contract(),
                            &format!("{context} continuation contract"),
                        )?,
                    ],
                )],
            ))
        }
    }
}

fn continuation_contract_shallow_text(
    lir: &LateLoweredProgram,
    types: &TypeStore,
    contract: scoopc_lir::effect_lowered::ir::LateLoweredContinuationContract,
    context: &str,
) -> Result<String, EffectLoweringError> {
    Ok(canonical_record(
        "continuation_contract_shallow",
        [
            canonical_type_fragment(
                types,
                contract.resume_tuple_ty(),
                &format!("{context} resume tuple"),
            )?,
            canonical_type_fragment(types, contract.answer_ty(), &format!("{context} answer"))?,
            step_schema_shallow_summary_text(
                lir,
                types,
                contract.out_step_schema(),
                &format!("{context} out-step summary"),
            )?,
            canonical_type_fragment(types, contract.surface_ty(), &format!("{context} surface"))?,
        ],
    ))
}

fn step_schema_shallow_summary_text(
    lir: &LateLoweredProgram,
    types: &TypeStore,
    step_schema: scoopc_lir::effect_facts::StepSchemaId,
    context: &str,
) -> Result<String, EffectLoweringError> {
    let step_type = lir.step_type(step_schema).ok_or_else(|| {
        invalid_lir_facts_error(format!(
            "{context} 缺少 step schema {} 对应的 late-lowered step type",
            step_schema.as_u32()
        ))
    })?;
    let mut case_fragments = step_type
        .cases()
        .iter()
        .map(|case| {
            Ok(canonical_record(
                "step_case_shallow",
                [
                    concrete_op_key_text(
                        types,
                        case.concrete_op_key(),
                        &format!("{context} concrete op"),
                    )?,
                    canonical_type_fragment(
                        types,
                        case.payload_tuple_ty(),
                        &format!("{context} payload tuple"),
                    )?,
                ],
            ))
        })
        .collect::<Result<Vec<_>, EffectLoweringError>>()?;
    case_fragments.sort();
    Ok(canonical_record(
        "step_schema_shallow",
        [
            canonical_type_fragment(
                types,
                step_type.invoke_args_tuple_ty(),
                &format!("{context} invoke args tuple"),
            )?,
            canonical_type_fragment(
                types,
                step_type.complete_ty(),
                &format!("{context} complete type"),
            )?,
            canonical_type_fragment(
                types,
                step_type.continuation_obj_ty(),
                &format!("{context} continuation object type"),
            )?,
            canonical_list(&case_fragments),
        ],
    ))
}

fn concrete_op_key_text(
    types: &TypeStore,
    concrete_op: &ConcreteOpKey,
    context: &str,
) -> Result<String, EffectLoweringError> {
    let instance = concrete_op.stable_instance_key().canonical_text();
    let mut family_type_args = concrete_op
        .effect_family()
        .type_args()
        .iter()
        .copied()
        .map(|ty| canonical_type_fragment(types, ty, &format!("{context} effect family arg")))
        .collect::<Result<Vec<_>, _>>()?;
    family_type_args.shrink_to_fit();
    Ok(canonical_record(
        "concrete_op",
        [
            instance,
            canonical_record(
                "effect_family",
                [
                    concrete_op.effect_family().effect_fqn().to_string(),
                    canonical_list(&family_type_args),
                ],
            ),
        ],
    ))
}

fn canonical_type_fragment(
    types: &TypeStore,
    ty: TypeId,
    context: &str,
) -> Result<String, EffectLoweringError> {
    canonical_type_text(types, ty, &NoTypeParamResolver).map_err(|error| {
        invalid_lir_facts_error(format!(
            "{context} 编码 canonical type text 失败（t{}）: {error}",
            ty.as_u32()
        ))
    })
}

fn canonical_effect_row_fragment(
    types: &TypeStore,
    row: &scoopc_lir::ty::EffectRow,
    context: &str,
) -> Result<String, EffectLoweringError> {
    canonical_effect_row_text(types, row, &NoTypeParamResolver).map_err(|error| {
        invalid_lir_facts_error(format!("{context} 编码 canonical effect row 失败: {error}"))
    })
}

fn invalid_lir_facts_error(detail: String) -> EffectLoweringError {
    EffectLoweringError::InvalidLirFactsContract { detail }
}

fn published_callable_key(
    callable: &LateLoweredCallable,
) -> Result<&StableLirCallableKey, EffectLoweringError> {
    callable
        .lir_callable_key()
        .ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
            detail: format!(
                "callable `{}` did not publish a stable LIR callable key",
                callable.root_fqn()
            ),
        })
}

fn published_body_version_key(
    callable: &LateLoweredCallable,
) -> Result<&BodyVersionKey, EffectLoweringError> {
    let key = callable.lir_body_version_key().ok_or_else(|| {
        EffectLoweringError::InvalidLirFactsContract {
            detail: format!(
                "callable `{}` did not publish a LIR body-version key",
                callable.root_fqn()
            ),
        }
    })?;
    let callable_key = published_callable_key(callable)?;
    if key.owner_canonical_text() != callable_key.as_str() {
        return Err(EffectLoweringError::InvalidLirFactsContract {
            detail: format!(
                "callable `{}` body-version owner `{}` does not match LIR key `{}`",
                callable.root_fqn(),
                key.owner_canonical_text(),
                callable_key.as_str(),
            ),
        });
    }
    Ok(key)
}

fn derived_body_version_key(
    callable: &LateLoweredCallable,
    key: &StableLirCallableKey,
) -> BodyVersionKey {
    let role = format!(
        "{:?}:impl={:?}:reentry={}",
        callable.call_abi_kind(),
        callable.impl_plan(),
        callable.needs_reentry()
    );
    BodyVersionKey::new(key, role, 0)
}

fn body_version_facts(
    callable: &LateLoweredCallable,
) -> Result<LirBodyVersionFacts, EffectLoweringError> {
    let key = published_body_version_key(callable)?.clone();
    Ok(LirBodyVersionFacts {
        key,
        impl_plan: format!("{:?}", callable.impl_plan()),
        needs_reentry: callable.needs_reentry(),
        allowed_effect_terms: callable.allowed_row().terms.clone(),
    })
}

fn build_global_init_facts(
    ctx: &LirFactsBuildContext<'_>,
) -> Result<LirGlobalInitFacts, EffectLoweringError> {
    let mut facts = LirGlobalInitFacts::default();

    for root in &ctx.mir_facts.roots.initializers {
        let root_facts = initializer_global_root_facts(ctx, root)?;
        publish_global_root_contracts(&mut facts, root_facts);
    }
    for root in &ctx.mir_facts.roots.extern_globals {
        let root_facts = extern_global_root_facts(ctx, root)?;
        facts.roots.insert(root_facts.root.clone(), root_facts);
    }

    publish_cone_init_routines(&mut facts)?;
    Ok(facts)
}

fn publish_global_root_contracts(facts: &mut LirGlobalInitFacts, root: LirGlobalRootFacts) {
    match root.kind {
        LirGlobalRootKind::TopLevelImmutableVal | LirGlobalRootKind::TopLevelMutableVar => {
            facts.top_level_eager_inits.insert(
                root.root.clone(),
                LirTopLevelEagerInitFacts {
                    root: root.root.clone(),
                    storage: root.storage,
                    has_initializer: root.has_initializer,
                },
            );
        }
        LirGlobalRootKind::ObjectSingleton => {
            facts.object_once.insert(
                root.root.clone(),
                LirObjectOnceFacts {
                    root: root.root.clone(),
                    has_initializer: root.has_initializer,
                },
            );
        }
        LirGlobalRootKind::ExternGlobal => {}
    }
    facts.roots.insert(root.root.clone(), root);
}

fn initializer_global_root_facts(
    ctx: &LirFactsBuildContext<'_>,
    root: &MirRootFact,
) -> Result<LirGlobalRootFacts, EffectLoweringError> {
    let MirRootDetail::Initializer {
        kind,
        has_initializer,
        dependency_count,
    } = &root.detail
    else {
        return invalid_lir_facts(format!(
            "MIR root `{}` is in initializer inventory but has non-initializer detail",
            root.fqn
        ));
    };
    let dependencies = ctx
        .mir_facts
        .roots
        .initializer_dependencies_for(root.fqn.as_str())
        .map(|dependency| LirGlobalRootDependency {
            target: LirGlobalRootKey::new(dependency.target_fqn.clone()),
            kind: global_dependency_kind(dependency.kind),
        })
        .collect::<Vec<_>>();
    if dependencies.len() != *dependency_count {
        return invalid_lir_facts(format!(
            "MIR initializer root `{}` reports {dependency_count} dependencies but publishes {} dependency identities",
            root.fqn,
            dependencies.len()
        ));
    }

    let (kind, storage) = initializer_root_kind(*kind);
    let initializer_body = if *has_initializer {
        Some(initializer_body_facts(ctx, &root.fqn, kind)?)
    } else {
        None
    };
    Ok(LirGlobalRootFacts {
        root: LirGlobalRootKey::new(root.fqn.clone()),
        kind,
        cone: root.identity.cone.clone(),
        source_cone_order: root.source_cone_order.unwrap_or(u32::MAX),
        ty: root.ty,
        storage,
        has_initializer: *has_initializer,
        dependencies,
        source_path: root.source_path.clone(),
        extern_global: None,
        initializer_body,
    })
}

fn extern_global_root_facts(
    ctx: &LirFactsBuildContext<'_>,
    root: &MirRootFact,
) -> Result<LirGlobalRootFacts, EffectLoweringError> {
    let MirRootDetail::ExternGlobal {
        storage,
        mutable,
        symbol,
        initializer_absent,
        unsafe_required,
    } = &root.detail
    else {
        return invalid_lir_facts(format!(
            "MIR root `{}` is in extern-global inventory but has non-extern detail",
            root.fqn
        ));
    };

    Ok(LirGlobalRootFacts {
        root: LirGlobalRootKey::new(root.fqn.clone()),
        kind: LirGlobalRootKind::ExternGlobal,
        cone: root.identity.cone.clone(),
        source_cone_order: root.source_cone_order.unwrap_or(u32::MAX),
        ty: root.ty,
        storage: Some(global_storage_policy(*storage)),
        has_initializer: !initializer_absent,
        dependencies: Vec::new(),
        source_path: root.source_path.clone(),
        extern_global: Some(LirExternGlobalFacts {
            symbol: symbol.clone(),
            linkage: extern_global_linkage(ctx.materialized, &root.fqn)?,
            mutable: *mutable,
            initializer_absent: *initializer_absent,
            unsafe_required: *unsafe_required,
        }),
        initializer_body: None,
    })
}

fn initializer_body_facts(
    ctx: &LirFactsBuildContext<'_>,
    root_fqn: &str,
    kind: LirGlobalRootKind,
) -> Result<LirInitializerBodyFacts, EffectLoweringError> {
    let contracts = ctx.materialized.backend_contracts();
    match kind {
        LirGlobalRootKind::TopLevelImmutableVal => {
            let value = contracts
                .top_level_immutable_values
                .get(root_fqn)
                .ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
                    detail: format!(
                        "missing top-level immutable initializer source contract for `{root_fqn}`"
                    ),
                })?;
            let span = value
                .init
                .as_ref()
                .map(|init| init.span)
                .unwrap_or(value.span);
            Ok(LirInitializerBodyFacts {
                root: LirGlobalRootKey::new(root_fqn.to_string()),
                kind: LirInitializerBodyKind::TopLevelImmutableVal,
                source_path: value.source_path.display().to_string(),
                source_span_start: span.start,
                source_span_end: span.end,
                body_item_count: usize::from(value.init.is_some()),
            })
        }
        LirGlobalRootKind::TopLevelMutableVar => {
            let var = contracts.top_level_vars.get(root_fqn).ok_or_else(|| {
                EffectLoweringError::InvalidLirFactsContract {
                    detail: format!(
                        "missing top-level var initializer source contract for `{root_fqn}`"
                    ),
                }
            })?;
            let span = var.init.as_ref().map(|init| init.span).unwrap_or(var.span);
            Ok(LirInitializerBodyFacts {
                root: LirGlobalRootKey::new(root_fqn.to_string()),
                kind: LirInitializerBodyKind::TopLevelMutableVar,
                source_path: var.source_path.display().to_string(),
                source_span_start: span.start,
                source_span_end: span.end,
                body_item_count: usize::from(var.init.is_some()),
            })
        }
        LirGlobalRootKind::ObjectSingleton => {
            let object = contracts.object_inits.get(root_fqn).ok_or_else(|| {
                EffectLoweringError::InvalidLirFactsContract {
                    detail: format!("missing object initializer source contract for `{root_fqn}`"),
                }
            })?;
            Ok(LirInitializerBodyFacts {
                root: LirGlobalRootKey::new(root_fqn.to_string()),
                kind: LirInitializerBodyKind::ObjectSingleton,
                source_path: object.source_path.display().to_string(),
                source_span_start: object.span.start,
                source_span_end: object.span.end,
                body_item_count: object.steps.len(),
            })
        }
        LirGlobalRootKind::ExternGlobal => invalid_lir_facts(format!(
            "extern global `{root_fqn}` cannot publish initializer body facts"
        )),
    }
}

fn publish_cone_init_routines(facts: &mut LirGlobalInitFacts) -> Result<(), EffectLoweringError> {
    let mut roots_by_cone = BTreeMap::<(u32, String), Vec<LirGlobalRootKey>>::new();
    for (root_key, root) in &facts.roots {
        if facts.top_level_eager_inits.contains_key(root_key) {
            roots_by_cone
                .entry((root.source_cone_order, root.cone.canonical_text()))
                .or_default()
                .push(root_key.clone());
        }
    }

    let mut root_to_routine = BTreeMap::<LirGlobalRootKey, LirConeInitRoutineKey>::new();
    for (index, (_cone_key, roots)) in roots_by_cone.into_iter().enumerate() {
        let ordered_roots = topologically_order_eager_roots(facts, roots)?;
        let Some(first_root) = ordered_roots.first().and_then(|root| facts.roots.get(root)) else {
            continue;
        };
        let routine = LirConeInitRoutineKey::new(index as u32);
        for root in &ordered_roots {
            root_to_routine.insert(root.clone(), routine);
        }
        facts.cone_init_routines.insert(
            routine,
            LirConeInitRoutineFacts {
                routine,
                cone: first_root.cone.clone(),
                source_cone_order: first_root.source_cone_order,
                roots: ordered_roots,
            },
        );
    }
    facts.final_entry_order.routines =
        topologically_order_cone_init_routines(facts, &root_to_routine)?;
    Ok(())
}

fn topologically_order_cone_init_routines(
    facts: &LirGlobalInitFacts,
    root_to_routine: &BTreeMap<LirGlobalRootKey, LirConeInitRoutineKey>,
) -> Result<Vec<LirConeInitRoutineKey>, EffectLoweringError> {
    let mut pending = facts
        .cone_init_routines
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::new();

    while !pending.is_empty() {
        let ready = pending
            .iter()
            .find(|routine| {
                cone_init_routine_dependencies_ready(facts, root_to_routine, **routine, &ordered)
            })
            .copied();
        let Some(routine) = ready else {
            let cycle = pending
                .iter()
                .map(|routine| format!("routine#{}", routine.as_u32()))
                .collect::<Vec<_>>()
                .join(", ");
            return invalid_lir_facts(format!(
                "cyclic cross-cone top-level eager init dependencies among: {cycle}"
            ));
        };
        pending.remove(&routine);
        ordered.push(routine);
    }

    Ok(ordered)
}

fn cone_init_routine_dependencies_ready(
    facts: &LirGlobalInitFacts,
    root_to_routine: &BTreeMap<LirGlobalRootKey, LirConeInitRoutineKey>,
    routine: LirConeInitRoutineKey,
    ordered: &[LirConeInitRoutineKey],
) -> bool {
    let Some(routine_facts) = facts.cone_init_routines.get(&routine) else {
        return false;
    };
    routine_facts.roots.iter().all(|root| {
        let Some(root_facts) = facts.roots.get(root) else {
            return false;
        };
        root_facts.dependencies.iter().all(|dependency| {
            let Some(dependency_routine) = root_to_routine.get(&dependency.target) else {
                return true;
            };
            *dependency_routine == routine || ordered.contains(dependency_routine)
        })
    })
}

fn topologically_order_eager_roots(
    facts: &LirGlobalInitFacts,
    roots: Vec<LirGlobalRootKey>,
) -> Result<Vec<LirGlobalRootKey>, EffectLoweringError> {
    let eager_roots = roots.iter().cloned().collect::<BTreeSet<_>>();
    let mut pending = eager_roots.clone();
    let mut ordered = Vec::new();

    while !pending.is_empty() {
        let ready = pending
            .iter()
            .find(|root| {
                let Some(root_facts) = facts.roots.get(*root) else {
                    return false;
                };
                root_facts.dependencies.iter().all(|dependency| {
                    !eager_roots.contains(&dependency.target)
                        || ordered.contains(&dependency.target)
                })
            })
            .cloned();
        let Some(root) = ready else {
            let cycle = pending
                .iter()
                .map(|root| root.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return invalid_lir_facts(format!(
                "cyclic top-level eager init dependencies among: {cycle}"
            ));
        };
        pending.remove(&root);
        ordered.push(root);
    }

    Ok(ordered)
}

fn initializer_root_kind(
    kind: MirInitializerRootKind,
) -> (LirGlobalRootKind, Option<LirGlobalStoragePolicy>) {
    match kind {
        MirInitializerRootKind::RuntimeImmutableVal => {
            (LirGlobalRootKind::TopLevelImmutableVal, None)
        }
        MirInitializerRootKind::RuntimeMutableGlobalVar => (
            LirGlobalRootKind::TopLevelMutableVar,
            Some(LirGlobalStoragePolicy::Global),
        ),
        MirInitializerRootKind::RuntimeMutableThreadLocalVar => (
            LirGlobalRootKind::TopLevelMutableVar,
            Some(LirGlobalStoragePolicy::ThreadLocal),
        ),
        MirInitializerRootKind::ObjectSingleton => (LirGlobalRootKind::ObjectSingleton, None),
    }
}

fn global_storage_policy(storage: MirGlobalStorageKind) -> LirGlobalStoragePolicy {
    match storage {
        MirGlobalStorageKind::Global => LirGlobalStoragePolicy::Global,
        MirGlobalStorageKind::ThreadLocal => LirGlobalStoragePolicy::ThreadLocal,
    }
}

fn extern_global_linkage(
    materialized: &MaterializedMir,
    root_fqn: &str,
) -> Result<LirExternGlobalLinkage, EffectLoweringError> {
    let linkage = materialized.file.items.iter().find_map(|item| match item {
        Item::ExternGlobal(global) if global.fqn == root_fqn => Some(global.linkage),
        _ => None,
    });
    match linkage {
        Some(crate::hir::ExternGlobalLinkage::External) => Ok(LirExternGlobalLinkage::External),
        None => invalid_lir_facts(format!(
            "missing materialized extern global linkage for `{root_fqn}`"
        )),
    }
}

fn global_dependency_kind(kind: MirInitializerDependencyKind) -> LirGlobalDependencyKind {
    match kind {
        MirInitializerDependencyKind::TopLevelValue => LirGlobalDependencyKind::TopLevelValue,
        MirInitializerDependencyKind::ObjectSingleton => LirGlobalDependencyKind::ObjectSingleton,
    }
}

fn build_physical_layout_facts(
    ctx: &LirFactsBuildContext<'_>,
    callables: &BTreeMap<StableLirCallableKey, LirCallableFacts>,
    dynamic_invokes: &BTreeMap<LirDynamicInvokeKey, LirDynamicInvokeContract>,
    source_signatures: &BTreeMap<String, LirSourceCallableSignatureFacts>,
) -> Result<LirPhysicalLayoutFacts, EffectLoweringError> {
    let contracts = ctx.materialized.backend_contracts();
    let mut facts = LirPhysicalLayoutFacts::default();

    for (layout_key, class) in &contracts.class_inits {
        insert_class_layout_fact(&mut facts, layout_key.as_str(), class);
    }
    for (fqn, layout) in &contracts.enum_layouts {
        facts.enums.insert(fqn.clone(), enum_layout_facts(layout));
    }
    insert_builtin_option_enum_layout_facts(&mut facts, ctx.effect_facts.types());
    for (class_fqn, slots) in &contracts.class_vtables {
        facts.class_vtables.insert(
            class_fqn.clone(),
            slots
                .iter()
                .map(|slot| LirClassVtableSlotFacts {
                    slot: slot.slot,
                    name: slot.name.clone(),
                    params_len: slot.params_len,
                    has_receiver: slot.has_receiver,
                    impl_member_fqn: slot.impl_member_fqn.clone(),
                })
                .collect(),
        );
    }
    for (fqn, interface) in &contracts.interfaces {
        facts.interfaces.insert(
            fqn.clone(),
            LirInterfaceLayoutFacts {
                fqn: interface.fqn.clone(),
                interface_id: interface.interface_id,
                super_interfaces: interface.super_interfaces.clone(),
                method_slots: interface
                    .method_slots
                    .iter()
                    .map(|slot| LirInterfaceMethodSlotFacts {
                        slot: slot.slot,
                        name: slot.name.clone(),
                        member_fqn: slot.member_fqn.clone(),
                        params_len: slot.params_len,
                        has_receiver: slot.has_receiver,
                        has_body: slot.has_body,
                    })
                    .collect(),
            },
        );
    }
    for (class_fqn, entries) in &contracts.class_itables {
        facts.class_itables.insert(
            class_fqn.clone(),
            LirClassItableFacts {
                class_fqn: class_fqn.clone(),
                entries: entries
                    .iter()
                    .map(|entry| LirClassItableEntryFacts {
                        interface_fqn: entry.interface_fqn.clone(),
                        interface_id: entry.interface_id,
                        interface_type_name: entry.interface_type_name.clone(),
                        interface_type_id: entry.interface_type_id,
                        runtime_match_type_names: entry.runtime_match_type_names.clone(),
                        runtime_match_type_ids: entry.runtime_match_type_ids.clone(),
                        method_impl_fqns: entry.method_impl_fqns.clone(),
                        method_receiver_type_ids: entry.method_receiver_type_ids.clone(),
                    })
                    .collect(),
            },
        );
    }
    facts.callable_symbols = build_callable_symbol_facts(ctx)?;
    facts.abi_symbols = build_abi_symbol_facts(
        &facts.callable_symbols,
        &contracts.native_callable_funs,
        &contracts.extern_funs,
        &contracts.class_vtables,
        &contracts.class_itables,
        source_signatures,
        callables,
        dynamic_invokes,
    );
    facts.layout_names = build_layout_name_facts(&facts);
    facts.closure_identities = build_closure_identity_facts(ctx)?;
    Ok(facts)
}

#[allow(clippy::too_many_arguments)]
fn build_abi_symbol_facts(
    callable_symbols: &BTreeMap<StableLirCallableKey, LirCallableSymbolFacts>,
    native_callable_funs: &crate::hir::NativeCallableFunIndex,
    extern_funs: &crate::hir::ExternFunIndex,
    class_vtables: &crate::vtable::ClassVtableIndex,
    class_itables: &crate::itable::ClassItableIndex,
    source_signatures: &BTreeMap<String, LirSourceCallableSignatureFacts>,
    callables: &BTreeMap<StableLirCallableKey, LirCallableFacts>,
    dynamic_invokes: &BTreeMap<LirDynamicInvokeKey, LirDynamicInvokeContract>,
) -> BTreeMap<String, LirAbiSymbolFact> {
    let mut out = BTreeMap::new();
    let callable_roots = callable_symbols
        .values()
        .map(|symbol| symbol.root_fqn.as_str())
        .collect::<BTreeSet<_>>();
    for (key, symbol) in callable_symbols {
        if let Some(exported) = &symbol.exported_symbol {
            insert_abi_symbol_fact(
                &mut out,
                key.canonical_text(),
                exported.clone(),
                Some(key.clone()),
                Some(symbol.root_fqn.clone()),
                "callable_export",
            );
        }
        if let Some(native) = &symbol.native {
            insert_abi_symbol_fact(
                &mut out,
                key.canonical_text(),
                native.symbol.clone(),
                Some(key.clone()),
                Some(symbol.root_fqn.clone()),
                "native_callable",
            );
        }
        if let Some(extern_) = &symbol.extern_ {
            insert_abi_symbol_fact(
                &mut out,
                key.canonical_text(),
                extern_.symbol.clone(),
                Some(key.clone()),
                Some(symbol.root_fqn.clone()),
                "extern_callable",
            );
        }
    }
    for (root_fqn, native) in native_callable_funs {
        if callable_roots.contains(root_fqn.as_str()) {
            continue;
        }
        insert_abi_symbol_fact(
            &mut out,
            root_fqn.clone(),
            native.symbol.clone(),
            None,
            Some(root_fqn.clone()),
            "native_callable",
        );
    }
    for (root_fqn, extern_fun) in extern_funs {
        if callable_roots.contains(root_fqn.as_str()) {
            continue;
        }
        insert_abi_symbol_fact(
            &mut out,
            root_fqn.clone(),
            extern_fun.symbol.clone(),
            None,
            Some(root_fqn.clone()),
            "extern_callable",
        );
    }
    for exact in exact_callee_bindings(callables, dynamic_invokes) {
        let role =
            if callable_symbol_contains_canonical_key(callable_symbols, &exact.target_callable_key)
            {
                "callable_export"
            } else if native_callable_funs.contains_key(exact.root_fqn.as_str()) {
                "native_callable"
            } else if extern_funs.contains_key(exact.root_fqn.as_str()) {
                "extern_callable"
            } else {
                "callable_export"
            };
        if out.values().any(|fact| {
            fact.callable.as_ref() == Some(&exact.target_callable_key)
                && fact.root_fqn.as_deref() == Some(exact.root_fqn.as_str())
                && fact.symbol == exact.abi_symbol
                && fact.role == role
        }) {
            continue;
        }
        insert_abi_symbol_fact(
            &mut out,
            exact.target_callable_key.canonical_text(),
            exact.abi_symbol.clone(),
            Some(exact.target_callable_key.clone()),
            Some(exact.root_fqn.clone()),
            role,
        );
    }
    for root_fqn in dispatch_layout_impl_roots(class_vtables, class_itables) {
        insert_declaration_abi_symbol_if_missing(&mut out, root_fqn);
    }
    for root_fqn in source_signatures.keys() {
        insert_declaration_abi_symbol_if_missing(&mut out, root_fqn.clone());
    }
    out
}

fn insert_declaration_abi_symbol_if_missing(
    out: &mut BTreeMap<String, LirAbiSymbolFact>,
    root_fqn: String,
) {
    if root_fqn.is_empty()
        || out
            .values()
            .any(|fact| fact.root_fqn.as_deref() == Some(root_fqn.as_str()))
    {
        return;
    }
    let declaration_key = StableLirCallableKey::new(
        canonical_record("lir_callable_declaration", [root_fqn.clone()]),
        root_fqn.clone(),
    );
    insert_abi_symbol_fact(
        out,
        declaration_key.canonical_text(),
        AbiMangler.fun_symbol(&declaration_key),
        Some(declaration_key),
        Some(root_fqn),
        "callable_export",
    );
}

fn dispatch_layout_impl_roots(
    class_vtables: &crate::vtable::ClassVtableIndex,
    class_itables: &crate::itable::ClassItableIndex,
) -> BTreeSet<String> {
    let mut roots = BTreeSet::new();
    for slot in class_vtables.values().flat_map(|slots| slots.iter()) {
        if !slot.impl_member_fqn.is_empty() {
            roots.insert(slot.impl_member_fqn.clone());
        }
    }
    for entry in class_itables.values().flat_map(|entries| entries.iter()) {
        for impl_fqn in &entry.method_impl_fqns {
            if !impl_fqn.is_empty() {
                roots.insert(impl_fqn.clone());
            }
        }
    }
    roots
}

fn callable_symbol_contains_canonical_key(
    callable_symbols: &BTreeMap<StableLirCallableKey, LirCallableSymbolFacts>,
    key: &StableLirCallableKey,
) -> bool {
    let canonical = key.canonical_text();
    callable_symbols
        .keys()
        .any(|published| published.canonical_text() == canonical)
}

fn exact_callee_bindings<'a>(
    callables: &'a BTreeMap<StableLirCallableKey, LirCallableFacts>,
    dynamic_invokes: &'a BTreeMap<LirDynamicInvokeKey, LirDynamicInvokeContract>,
) -> Vec<&'a LirExactCalleeBinding> {
    let mut out = Vec::new();
    for callable in callables.values() {
        if let LirCallableContract::Plain(plain) = &callable.contract {
            out.extend(
                plain
                    .call_sites
                    .iter()
                    .filter_map(|site| site.contract.exact_callee.as_ref()),
            );
        }
    }
    out.extend(
        dynamic_invokes
            .values()
            .filter_map(|contract| contract.call.exact_callee.as_ref()),
    );
    out
}

fn insert_abi_symbol_fact(
    out: &mut BTreeMap<String, LirAbiSymbolFact>,
    identity: String,
    symbol: String,
    callable: Option<StableLirCallableKey>,
    root_fqn: Option<String>,
    role: &str,
) {
    let fact_key = canonical_record("abi_symbol", [identity, role.to_string()]);
    out.insert(
        fact_key.clone(),
        LirAbiSymbolFact {
            key: fact_key,
            symbol,
            callable,
            root_fqn,
            role: role.to_string(),
        },
    );
}

fn build_layout_name_facts(facts: &LirPhysicalLayoutFacts) -> BTreeMap<String, LirLayoutNameFact> {
    let mut out = BTreeMap::new();
    for key in facts.classes.keys() {
        insert_layout_name_fact(&mut out, "class", key);
    }
    for key in facts.enums.keys() {
        insert_layout_name_fact(&mut out, "enum", key);
    }
    for key in facts.interfaces.keys() {
        insert_layout_name_fact(&mut out, "interface", key);
    }
    for key in facts.class_vtables.keys() {
        insert_layout_name_fact(&mut out, "class_vtable", key);
    }
    for key in facts.class_itables.keys() {
        insert_layout_name_fact(&mut out, "class_itable", key);
    }
    out
}

fn insert_layout_name_fact(out: &mut BTreeMap<String, LirLayoutNameFact>, family: &str, key: &str) {
    let fact_key = canonical_record("layout_name", [family.to_string(), key.to_string()]);
    out.insert(
        fact_key.clone(),
        LirLayoutNameFact {
            key: fact_key,
            family: family.to_string(),
            layout_name: key.to_string(),
        },
    );
}

fn build_closure_identity_facts(
    ctx: &LirFactsBuildContext<'_>,
) -> Result<BTreeMap<StableLirCallableKey, LirClosureIdentityFact>, EffectLoweringError> {
    let mut out = BTreeMap::new();
    for callable in ctx.lir.callables() {
        if !callable
            .source_callable()
            .is_some_and(|source| source.name.starts_with("$lambda"))
        {
            continue;
        }
        let (owner_root_fqn, lexical_path) = closure_owner_and_lexical_path(callable.root_fqn())
            .ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
                detail: format!(
                    "closure callable `{}` 缺少可发布的 owner lexical path",
                    callable.root_fqn()
                ),
            })?;
        let owner = ctx
            .lir
            .callables()
            .iter()
            .find(|candidate| candidate.root_fqn() == owner_root_fqn)
            .ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
                detail: format!(
                    "closure callable `{}` 的 owner `{owner_root_fqn}` 未发布 LIR callable identity",
                    callable.root_fqn()
                ),
            })?;
        let key = published_callable_key(callable)?.clone();
        out.insert(
            key.clone(),
            LirClosureIdentityFact {
                callable: key,
                root_fqn: callable.root_fqn().to_string(),
                owner_callable: published_callable_key(owner)?.clone(),
                owner_root_fqn: owner_root_fqn.to_string(),
                lexical_path: lexical_path.to_string(),
            },
        );
    }
    Ok(out)
}

fn closure_owner_and_lexical_path(root_fqn: &str) -> Option<(&str, &str)> {
    let lambda_start = root_fqn.find(".$lambda")?;
    let owner = &root_fqn[..lambda_start];
    let lexical_path = &root_fqn[lambda_start + 1..];
    (!owner.is_empty() && !lexical_path.is_empty()).then_some((owner, lexical_path))
}

fn insert_class_layout_fact(
    facts: &mut LirPhysicalLayoutFacts,
    layout_key: &str,
    class: &crate::hir::MonoClassInit,
) {
    facts.classes.insert(
        layout_key.to_string(),
        LirClassLayoutFacts {
            fqn: class.fqn.clone(),
            layout_key: layout_key.to_string(),
            super_class_fqn: class.super_class_fqn.clone(),
            fields: class
                .fields
                .iter()
                .map(|field| LirClassFieldFacts {
                    fqn: field.fqn.clone(),
                    name: field.name.clone(),
                    mutable: field.mutable,
                    ty: field.ty.inner(),
                })
                .collect(),
        },
    );
}

fn enum_layout_facts(layout: &crate::hir::EnumLayout) -> LirEnumLayoutFacts {
    LirEnumLayoutFacts {
        fqn: layout.fqn.clone(),
        repr: match &layout.repr {
            crate::hir::EnumRepr::TaggedUnion => LirEnumReprFacts::TaggedUnion,
            crate::hir::EnumRepr::ValueOnly { underlying_ty_fqn } => LirEnumReprFacts::ValueOnly {
                underlying_ty_fqn: underlying_ty_fqn.clone(),
            },
        },
        variants: layout
            .variants
            .iter()
            .map(|variant| LirEnumVariantFacts {
                name: variant.name.clone(),
                tag: variant.tag,
                fields: variant
                    .fields
                    .iter()
                    .map(|field| LirEnumVariantFieldFacts {
                        name: field.name.clone(),
                        ty: field.ty.map(|ty| ty.inner()),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn insert_builtin_option_enum_layout_facts(facts: &mut LirPhysicalLayoutFacts, types: &TypeStore) {
    for ty in types.iter_ids() {
        let TypeKind::Value(ValueTypeKind::Option(inner)) = types.kind(ty) else {
            continue;
        };
        let key = lir_nominal_layout_key("scoop.core.Option", &[*inner], types);
        facts
            .enums
            .entry(key.clone())
            .or_insert_with(|| LirEnumLayoutFacts {
                fqn: key,
                repr: LirEnumReprFacts::TaggedUnion,
                variants: vec![
                    LirEnumVariantFacts {
                        name: "Some".to_string(),
                        tag: 0,
                        fields: vec![LirEnumVariantFieldFacts {
                            name: "value".to_string(),
                            ty: Some(*inner),
                        }],
                    },
                    LirEnumVariantFacts {
                        name: "None".to_string(),
                        tag: 1,
                        fields: Vec::new(),
                    },
                ],
            });
    }
}

fn lir_nominal_layout_key(fqn: &str, args: &[TypeId], types: &TypeStore) -> String {
    if args.is_empty() {
        return fqn.to_string();
    }
    let arg_text = args
        .iter()
        .map(|id| types.display(*id).to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("{fqn}<{arg_text}>")
}

fn build_callable_symbol_facts(
    ctx: &LirFactsBuildContext<'_>,
) -> Result<BTreeMap<StableLirCallableKey, LirCallableSymbolFacts>, EffectLoweringError> {
    let contracts = ctx.materialized.backend_contracts();
    let mut out = BTreeMap::new();
    for callable in ctx.lir.callables() {
        let key = published_callable_key(callable)?.clone();
        let signature = callable_signature(callable)?;
        let native = contracts
            .native_callable_funs
            .get(callable.root_fqn())
            .map(|native| LirNativeCallableSignatureFacts {
                symbol: native.symbol.clone(),
                calling_convention: native.calling_convention.clone(),
                param_names: signature.param_names.clone(),
                param_tys: signature.param_tys.clone(),
                return_ty: signature.return_ty,
            });
        let extern_ = contracts
            .extern_funs
            .get(callable.root_fqn())
            .map(|extern_fun| LirExternCallableSignatureFacts {
                symbol: extern_fun.symbol.clone(),
                abi: extern_fun.abi.name().to_string(),
                calling_convention: extern_fun.calling_convention.clone(),
                lib: extern_fun.lib.clone(),
                param_names: signature.param_names.clone(),
                param_tys: signature.param_tys.clone(),
                return_ty: signature.return_ty,
            });
        let exported_symbol = Some(AbiMangler.fun_symbol(&key));
        out.insert(
            key.clone(),
            LirCallableSymbolFacts {
                callable: key,
                root_fqn: callable.root_fqn().to_string(),
                stable_instance_key: callable.stable_instance_key().canonical_text(),
                exported_symbol,
                kind: callable_symbol_kind(callable, native.as_ref(), extern_.as_ref()),
                abi_kind: callable_abi_kind(callable.call_abi_kind()),
                param_names: signature.param_names,
                param_tys: signature.param_tys,
                return_ty: signature.return_ty,
                native,
                extern_,
            },
        );
    }
    Ok(out)
}

fn build_source_signature_facts(
    lir: &LateLoweredProgram,
    mir_facts: &MirFacts,
) -> Result<BTreeMap<String, LirSourceCallableSignatureFacts>, EffectLoweringError> {
    let mut out = BTreeMap::new();
    for signature in &mir_facts.backend.source_signatures {
        insert_source_signature_fact(
            &mut out,
            LirSourceCallableSignatureFacts {
                signature_key: source_callable_signature_key(&signature.fqn),
                root_fqn: signature.fqn.clone(),
                param_names: signature.param_names.clone(),
                param_tys: signature.param_tys.clone(),
                return_ty: signature.return_ty,
            },
        )?;
    }
    for callable in lir.callables() {
        insert_source_signature_fact(&mut out, source_signature_facts_for_callable(callable)?)?;
    }
    Ok(out)
}

fn source_signature_facts_for_callable(
    callable: &LateLoweredCallable,
) -> Result<LirSourceCallableSignatureFacts, EffectLoweringError> {
    let fun =
        callable
            .source_callable()
            .ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
                detail: format!(
                    "callable `{}` did not publish a source callable signature",
                    callable.root_fqn()
                ),
            })?;
    Ok(source_signature_facts_for_fun(fun))
}

fn source_signature_facts_for_fun(fun: &FunDecl) -> LirSourceCallableSignatureFacts {
    LirSourceCallableSignatureFacts {
        signature_key: source_callable_signature_key(&fun.fqn),
        root_fqn: fun.fqn.clone(),
        param_names: fun.params.iter().map(|param| param.name.clone()).collect(),
        param_tys: fun.params.iter().map(|param| param.ty).collect(),
        return_ty: fun.return_ty,
    }
}

fn source_callable_signature_key(root_fqn: &str) -> String {
    canonical_record("source_callable_signature", [root_fqn.to_string()])
}

fn callable_signature(
    callable: &LateLoweredCallable,
) -> Result<MaterializedCallableSignature, EffectLoweringError> {
    let fun =
        callable
            .source_callable()
            .ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
                detail: format!(
                    "callable `{}` did not publish source signature metadata",
                    callable.root_fqn()
                ),
            })?;
    let param_names = fun
        .params
        .iter()
        .map(|param| param.name.clone())
        .collect::<Vec<_>>();
    let param_tys = fun.params.iter().map(|param| param.ty).collect::<Vec<_>>();
    let skip_closure_env = usize::from(fun.name.starts_with("$lambda"));
    let closure_carrier_arg_tys = param_tys.iter().copied().skip(skip_closure_env).collect();
    Ok(MaterializedCallableSignature {
        source_kind: callable.source_kind(),
        param_names,
        param_tys,
        return_ty: fun.return_ty,
        closure_carrier_arg_tys,
    })
}

fn insert_source_signature_fact(
    out: &mut BTreeMap<String, LirSourceCallableSignatureFacts>,
    signature: LirSourceCallableSignatureFacts,
) -> Result<(), EffectLoweringError> {
    if out.contains_key(signature.root_fqn.as_str()) {
        return Ok(());
    }
    out.insert(signature.root_fqn.clone(), signature);
    Ok(())
}

fn build_class_ctor_init_facts(
    lir: &LateLoweredProgram,
) -> BTreeMap<LirClassCtorInitKey, LirClassCtorInitFacts> {
    let mut out = BTreeMap::new();
    for body in lir.class_ctor_init_bodies() {
        let key = body.key().clone();
        let ctor_span = body.ctor_span();
        let implicit_super = body.implicit_super().map(|super_call| {
            let source_span = super_call.source_span();
            LirClassCtorSuperCallFacts {
                target: super_call.target().clone(),
                class_fqn: super_call.class_fqn().to_string(),
                arg_count: super_call.args().len(),
                source_span_start: source_span.map(|span| span.start),
                source_span_end: source_span.map(|span| span.end),
            }
        });
        let delegation = body
            .delegation()
            .map(|delegation| LirClassCtorDelegationFacts {
                kind: delegation.kind(),
                target: delegation.target().clone(),
                class_fqn: delegation.class_fqn().to_string(),
                arg_count: delegation.args().len(),
                source_span_start: delegation.span().start,
                source_span_end: delegation.span().end,
            });
        let steps = body
            .steps()
            .iter()
            .map(|step| {
                let span = step.span();
                LirClassCtorInitStepFacts {
                    kind: step.kind(),
                    field_fqn: step.field_fqn().map(str::to_string),
                    source_span_start: span.start,
                    source_span_end: span.end,
                }
            })
            .collect();
        out.insert(
            key.clone(),
            LirClassCtorInitFacts {
                key,
                class_fqn: body.class_fqn().to_string(),
                source_path: body.source_path().display().to_string(),
                ctor_kind: body.ctor_kind(),
                ctor_span_start: ctor_span.map(|span| span.start),
                ctor_span_end: ctor_span.map(|span| span.end),
                params: body
                    .params()
                    .iter()
                    .map(|param| LirClassCtorParamFacts {
                        name: param.name().to_string(),
                        ty: param.ty().inner(),
                        has_default: param.default_value().is_some(),
                        is_property: param.is_property(),
                        property_field_fqn: param.property_field_fqn().map(str::to_string),
                    })
                    .collect(),
                implicit_super,
                delegation,
                steps,
            },
        );
    }
    out
}

fn callable_symbol_kind(
    callable: &LateLoweredCallable,
    native: Option<&LirNativeCallableSignatureFacts>,
    extern_: Option<&LirExternCallableSignatureFacts>,
) -> LirCallableSymbolKind {
    if native.is_some() {
        return LirCallableSymbolKind::NativeExtern;
    }
    if let Some(extern_) = extern_ {
        return if extern_.abi == "scoop" {
            LirCallableSymbolKind::ManagedExtern
        } else {
            LirCallableSymbolKind::NativeExtern
        };
    }
    if callable.call_abi_kind() == CallableAbiKind::EffectStep {
        LirCallableSymbolKind::EffectBridge
    } else {
        LirCallableSymbolKind::ManagedOrdinary
    }
}

fn build_type_context_facts(
    materialized: &MaterializedMir,
    effect_facts: &MaterializedEffectFacts,
) -> LirTypeContextFacts {
    let materialized_fingerprint = type_store_fingerprint(&materialized.types);
    let effect_facts_fingerprint = type_store_fingerprint(effect_facts.types());
    let fingerprints_match = materialized_fingerprint == effect_facts_fingerprint;
    LirTypeContextFacts {
        owner: LirTypeContextOwner::LirStageBaseContext,
        primary_fingerprint: materialized_fingerprint.clone(),
        materialized_fingerprint,
        effect_facts_fingerprint,
        bridge_mode: if fingerprints_match {
            LirTypeContextBridgeMode::Identical
        } else {
            LirTypeContextBridgeMode::ExplicitDisplayNameRemap
        },
        remapped_type_count: if fingerprints_match {
            0
        } else {
            effect_facts.types().iter_ids().count()
        },
        stable_wire_format: LirTypeStableWireFormatFacts {
            decision: LirTypeStableWireFormatDecision::Implemented,
            owner: "P10 TypeStore portable serialization".to_string(),
            reason: "Persisted facts and LIR retain TypeId fields but are paired with portable TypeStore serialization, so each process reconstructs an isomorphic local type universe before consuming them.".to_string(),
            non_blocking_reason: "Per-cone artifacts serialize type_store.bin next to facts and LIR; cached dependency handoff imports that TypeStore before ABI/layout queries.".to_string(),
        },
    }
}

fn type_store_fingerprint(types: &TypeStore) -> String {
    let mut entries = types
        .iter_ids()
        .map(|ty| format!("t{}={}", ty.as_u32(), types.display(ty)))
        .collect::<Vec<_>>();
    entries.sort();
    entries.join("|")
}

fn invalid_lir_facts<T>(detail: String) -> Result<T, EffectLoweringError> {
    Err(EffectLoweringError::InvalidLirFactsContract { detail })
}

fn build_callable_facts(
    ctx: &LirFactsBuildContext<'_>,
    dynamic_invokes: &mut BTreeMap<LirDynamicInvokeKey, LirDynamicInvokeContract>,
    dispatches: &mut BTreeMap<LirDispatchKey, LirDispatchContract>,
) -> Result<BTreeMap<StableLirCallableKey, LirCallableFacts>, EffectLoweringError> {
    let mut out = BTreeMap::new();
    for callable in ctx.lir.callables() {
        let key = published_callable_key(callable)?.clone();
        let contract = match callable.abi() {
            scoopc_lir::effect_lowered::ir::LateLoweredCallableAbi::Plain(plain) => {
                LirCallableContract::Plain(Box::new(build_plain_callable_facts(
                    ctx,
                    callable,
                    &key,
                    plain,
                    dynamic_invokes,
                    dispatches,
                )?))
            }
            scoopc_lir::effect_lowered::ir::LateLoweredCallableAbi::EffectStep(effect) => {
                LirCallableContract::EffectStep(Box::new(build_effect_step_callable_facts(
                    ctx,
                    callable,
                    &key,
                    effect,
                    dynamic_invokes,
                    dispatches,
                )?))
            }
        };
        let signature = callable_signature(callable)?;
        out.insert(
            key.clone(),
            LirCallableFacts {
                root_fqn: callable.root_fqn().to_string(),
                stable_instance_key: callable.stable_instance_key().canonical_text(),
                source_kind: signature.source_kind,
                param_names: signature.param_names,
                param_tys: signature.param_tys,
                return_ty: signature.return_ty,
                body_version: body_version_facts(callable)?,
                resolved_outward_cases: callable
                    .resolved_outward_cases()
                    .iter()
                    .map(|case| LirCaseKey::new(case.as_u32()))
                    .collect(),
                contract,
            },
        );
    }
    Ok(out)
}

fn build_plain_callable_facts(
    ctx: &LirFactsBuildContext<'_>,
    callable: &LateLoweredCallable,
    owner_key: &StableLirCallableKey,
    plain: &LateLoweredPlainCallable,
    dynamic_invokes: &mut BTreeMap<LirDynamicInvokeKey, LirDynamicInvokeContract>,
    dispatches: &mut BTreeMap<LirDispatchKey, LirDispatchContract>,
) -> Result<LirPlainCallableFacts, EffectLoweringError> {
    let local_effect_control = plain
        .local_effect_control()
        .map(|control| {
            build_control_body_facts(
                ctx,
                callable,
                owner_key,
                control.step_schema(),
                control.state_graph(),
                control.frame_schema(),
                control.boundary_map(),
                control.resume_state_map(),
                control.source_statement_classifications(),
                control.continuation_object(),
                control.resume_packings(),
                dynamic_invokes,
                dispatches,
            )
        })
        .transpose()?;
    let call_sites = plain
        .call_sites()
        .iter()
        .map(|site| {
            build_plain_call_site_facts(ctx, callable, owner_key, site, dynamic_invokes, dispatches)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LirPlainCallableFacts {
        function_ty: plain.function_ty(),
        param_names: callable
            .source_callable()
            .map(|source| {
                source
                    .params
                    .iter()
                    .map(|param| param.name.clone())
                    .collect()
            })
            .unwrap_or_default(),
        param_tys: plain.param_tys().to_vec(),
        return_ty: plain.return_ty(),
        body_slices: plain
            .body_slices()
            .iter()
            .map(|slice| LirPlainBodySliceFacts {
                source_slice: plain_slice_key(*slice),
            })
            .collect(),
        call_sites,
        local_effect_control,
    })
}

fn build_effect_step_callable_facts(
    ctx: &LirFactsBuildContext<'_>,
    callable: &LateLoweredCallable,
    owner_key: &StableLirCallableKey,
    effect: &LateLoweredEffectStepCallable,
    dynamic_invokes: &mut BTreeMap<LirDynamicInvokeKey, LirDynamicInvokeContract>,
    dispatches: &mut BTreeMap<LirDispatchKey, LirDispatchContract>,
) -> Result<LirEffectStepCallableFacts, EffectLoweringError> {
    let signature = callable_signature(callable)?;
    let control_body = build_control_body_facts(
        ctx,
        callable,
        owner_key,
        effect.step_schema(),
        effect.state_graph(),
        effect.frame_schema(),
        effect.boundary_map(),
        effect.resume_state_map(),
        effect.source_statement_classifications(),
        effect.continuation_object(),
        effect.resume_packings(),
        dynamic_invokes,
        dispatches,
    )?;
    Ok(LirEffectStepCallableFacts {
        param_tys: signature.param_tys,
        closure_carrier_arg_tys: signature.closure_carrier_arg_tys,
        step_schema: LirStepSchemaKey::new(effect.step_schema().as_u32()),
        dynamic_invoke_entry: dynamic_invoke_entry_facts(effect.dynamic_invoke_entry()),
        control_body,
    })
}

fn build_plain_call_site_facts(
    ctx: &LirFactsBuildContext<'_>,
    callable: &LateLoweredCallable,
    owner_key: &StableLirCallableKey,
    site: &LateLoweredPlainCallSite,
    dynamic_invokes: &mut BTreeMap<LirDynamicInvokeKey, LirDynamicInvokeContract>,
    dispatches: &mut BTreeMap<LirDispatchKey, LirDispatchContract>,
) -> Result<LirPlainCallSiteFacts, EffectLoweringError> {
    let metadata = site.metadata();
    let contract = call_site_contract(ctx, site.facts())?;
    let dispatch = publish_dispatch_contract(
        ctx,
        owner_key,
        site.site_id(),
        metadata,
        &contract,
        dispatches,
    )?;
    let dynamic_invoke = publish_dynamic_invoke_contract(
        ctx,
        owner_key,
        callable
            .body_step_schema()
            .map(|schema| LirStepSchemaKey::new(schema.as_u32())),
        site.site_id(),
        LirDynamicInvokeSource::PlainCallSite {
            source_slice: plain_slice_key(site.source_slice()),
            statement_index: site.statement_index(),
        },
        contract.clone(),
        metadata,
        dispatch.clone(),
        dynamic_invokes,
    )?;
    Ok(LirPlainCallSiteFacts {
        site_id: site.site_id(),
        source_slice: plain_slice_key(site.source_slice()),
        statement_index: site.statement_index(),
        contract,
        dynamic_invoke,
        dispatch,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_control_body_facts(
    ctx: &LirFactsBuildContext<'_>,
    callable: &LateLoweredCallable,
    owner_key: &StableLirCallableKey,
    step_schema: scoopc_lir::effect_facts::StepSchemaId,
    state_graph: &LateLoweredStateGraph,
    frame_schema: &LateLoweredFrameSchema,
    boundary_map: &LateLoweredBoundaryMap,
    resume_state_map: &LateLoweredResumeStateMap,
    classifications: &[scoopc_lir::effect_lowered::ir::LateLoweredSourceStatementClassification],
    continuation_object: scoopc_lir::effect_lowered::ir::ContinuationObjectId,
    resume_packings: &[scoopc_lir::effect_lowered::ir::ResumeInterfaceId],
    dynamic_invokes: &mut BTreeMap<LirDynamicInvokeKey, LirDynamicInvokeContract>,
    dispatches: &mut BTreeMap<LirDispatchKey, LirDispatchContract>,
) -> Result<LirControlBodyFacts, EffectLoweringError> {
    let mut boundary_dynamic_invokes = HashMap::new();
    let mut boundary_dispatches = HashMap::new();

    for boundary in boundary_map.entries() {
        let (
            LateLoweredBoundarySource::Site {
                site_id,
                kind: BoundarySiteKind::Call,
            },
            Some(LateLoweredBoundaryLowering::Call(lowering)),
        ) = (boundary.source(), boundary.lowering())
        else {
            continue;
        };
        let contract = call_site_contract(ctx, lowering.facts())?;
        let metadata = lowering.metadata();
        let dispatch =
            publish_dispatch_contract(ctx, owner_key, site_id, metadata, &contract, dispatches)?;
        if let Some(key) = dispatch.clone() {
            boundary_dispatches.insert(boundary.boundary_id(), key);
        }
        let dynamic = publish_dynamic_invoke_contract(
            ctx,
            owner_key,
            Some(LirStepSchemaKey::new(step_schema.as_u32())),
            site_id,
            LirDynamicInvokeSource::Boundary {
                boundary_id: LirBoundaryKey::new(boundary.boundary_id().as_u32()),
            },
            contract,
            metadata,
            dispatch,
            dynamic_invokes,
        )?;
        if let Some(key) = dynamic {
            boundary_dynamic_invokes.insert(boundary.boundary_id(), key);
        }
    }

    publish_source_slice_dynamic_invokes(
        ctx,
        callable,
        owner_key,
        step_schema,
        classifications,
        dynamic_invokes,
        dispatches,
    )?;

    Ok(LirControlBodyFacts {
        step_schema: LirStepSchemaKey::new(step_schema.as_u32()),
        state_graph: state_graph_facts(state_graph),
        frame_schema: frame_schema_facts(frame_schema),
        boundary_map: boundary_map_facts(
            boundary_map,
            &boundary_dynamic_invokes,
            &boundary_dispatches,
        ),
        resume_state_map: resume_state_map_facts(resume_state_map),
        source_statement_count: classifications.len(),
        continuation_object: LirContinuationObjectKey::new(continuation_object.as_u32()),
        resume_packings: resume_packings
            .iter()
            .map(|id| LirResumePackingKey::new(id.as_u32()))
            .collect(),
    })
}

#[allow(clippy::too_many_arguments)]
fn publish_source_slice_dynamic_invokes(
    ctx: &LirFactsBuildContext<'_>,
    callable: &LateLoweredCallable,
    owner_key: &StableLirCallableKey,
    owner_step_schema: scoopc_lir::effect_facts::StepSchemaId,
    classifications: &[scoopc_lir::effect_lowered::ir::LateLoweredSourceStatementClassification],
    dynamic_invokes: &mut BTreeMap<LirDynamicInvokeKey, LirDynamicInvokeContract>,
    dispatches: &mut BTreeMap<LirDispatchKey, LirDispatchContract>,
) -> Result<(), EffectLoweringError> {
    let body_facts = ctx
        .effect_facts
        .body(callable.instance_key())
        .ok_or_else(|| EffectLoweringError::MissingBodyFacts {
            root_fqn: callable.root_fqn().to_string(),
        })?;
    for classification in classifications {
        let scoopc_lir::effect_lowered::ir::LateLoweredSourceStatementClassificationKind::DynamicInvokeCall {
            site_id,
            metadata,
        } = classification.kind()
        else {
            continue;
        };
        let site =
            body_facts
                .site(site_id)
                .ok_or_else(|| EffectLoweringError::MissingSiteFacts {
                    root_fqn: callable.root_fqn().to_string(),
                    site_id: site_id.as_u32(),
                })?;
        let SiteEffectFacts::Call(call_facts) = site else {
            return Err(EffectLoweringError::UnexpectedSiteFactsKind {
                root_fqn: callable.root_fqn().to_string(),
                site_id: site_id.as_u32(),
                expected: "Call",
                actual: scoopc_lir::effect_lowered::materialize::dispatch_plan::site_facts_kind(
                    site,
                ),
            });
        };
        let contract = call_site_contract(ctx, call_facts)?;
        let dispatch =
            publish_dispatch_contract(ctx, owner_key, site_id, &metadata, &contract, dispatches)?;
        publish_dynamic_invoke_contract(
            ctx,
            owner_key,
            Some(LirStepSchemaKey::new(owner_step_schema.as_u32())),
            site_id,
            LirDynamicInvokeSource::ControlSourceSlice {
                source_slice: state_slice_key(classification.source_slice()),
                statement_index: classification.statement_index(),
            },
            contract,
            &metadata,
            dispatch,
            dynamic_invokes,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn publish_dynamic_invoke_contract(
    ctx: &LirFactsBuildContext<'_>,
    owner_key: &StableLirCallableKey,
    owner_step_schema: Option<LirStepSchemaKey>,
    site_id: SiteId,
    source: LirDynamicInvokeSource,
    contract: LirCallSiteContract,
    metadata: &LateLoweredCallSiteMaterializedMetadata,
    dispatch: Option<LirDispatchKey>,
    dynamic_invokes: &mut BTreeMap<LirDynamicInvokeKey, LirDynamicInvokeContract>,
) -> Result<Option<LirDynamicInvokeKey>, EffectLoweringError> {
    if contract.callee_step_schema.is_none() || !is_dynamic_call_metadata(metadata) {
        return Ok(None);
    }
    let key = LirDynamicInvokeKey {
        owner_callable: owner_key.clone(),
        site_id,
    };
    if dynamic_invokes.contains_key(&key) {
        return Ok(Some(key));
    }
    let carrier =
        dynamic_invoke_carrier_contract(metadata.kind(), metadata.carrier_source_ty(), dispatch)?;
    let target_body_versions = contract
        .target_callables
        .iter()
        .filter_map(|target| ctx.body_versions_by_key.get(target).cloned())
        .collect();
    dynamic_invokes.insert(
        key.clone(),
        LirDynamicInvokeContract {
            owner_callable: owner_key.clone(),
            owner_step_schema,
            site_id,
            source,
            call: contract,
            carrier,
            arg_count: metadata.arg_count(),
            target_body_versions,
        },
    );
    Ok(Some(key))
}

fn publish_dispatch_contract(
    ctx: &LirFactsBuildContext<'_>,
    owner_key: &StableLirCallableKey,
    site_id: SiteId,
    metadata: &LateLoweredCallSiteMaterializedMetadata,
    contract: &LirCallSiteContract,
    dispatches: &mut BTreeMap<LirDispatchKey, LirDispatchContract>,
) -> Result<Option<LirDispatchKey>, EffectLoweringError> {
    let key = LirDispatchKey {
        owner_callable: owner_key.clone(),
        site_id,
    };
    if dispatches.contains_key(&key) {
        return Ok(Some(key));
    }
    let Some(dispatch) = dispatch_contract(ctx, owner_key, site_id, metadata, contract)? else {
        return Ok(None);
    };
    dispatches.insert(key.clone(), dispatch);
    Ok(Some(key))
}

fn dispatch_contract(
    ctx: &LirFactsBuildContext<'_>,
    owner_key: &StableLirCallableKey,
    site_id: SiteId,
    metadata: &LateLoweredCallSiteMaterializedMetadata,
    contract: &LirCallSiteContract,
) -> Result<Option<LirDispatchContract>, EffectLoweringError> {
    let facts = ctx.materialized.dispatch_devirtualization_facts();
    match metadata.kind() {
        LateLoweredCallSiteMaterializedKind::Virtual {
            owner_fqn,
            member_name,
            member_fqn,
            receiver_ty,
        } => {
            let dispatch = dispatch_metadata(owner_fqn, member_name, member_fqn, *receiver_ty);
            let method_slot = facts
                .virtual_method_slot(&dispatch, metadata.arg_count())
                .ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
                    detail: format!(
                        "virtual dispatch {}.{} at site{} has no unique vtable slot",
                        owner_fqn,
                        member_name,
                        site_id.as_u32()
                    ),
                })?;
            Ok(Some(LirDispatchContract {
                owner_callable: owner_key.clone(),
                site_id,
                kind: LirCallSiteKind::Virtual,
                owner_fqn: owner_fqn.clone(),
                member_name: member_name.clone(),
                member_fqn: member_fqn.clone(),
                receiver_ty: *receiver_ty,
                explicit_arg_count: metadata.arg_count(),
                method_slot,
                interface_id: None,
                candidate_targets: contract.target_callables.clone(),
            }))
        }
        LateLoweredCallSiteMaterializedKind::Interface {
            owner_fqn,
            member_name,
            member_fqn,
            receiver_ty,
        } => {
            let dispatch = dispatch_metadata(owner_fqn, member_name, member_fqn, *receiver_ty);
            let (interface_id, method_slot) = facts
                .interface_method_slot(&dispatch, metadata.arg_count())
                .ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
                    detail: format!(
                        "interface dispatch {} at site{} has no unique itable slot",
                        member_fqn,
                        site_id.as_u32()
                    ),
                })?;
            Ok(Some(LirDispatchContract {
                owner_callable: owner_key.clone(),
                site_id,
                kind: LirCallSiteKind::Interface,
                owner_fqn: owner_fqn.clone(),
                member_name: member_name.clone(),
                member_fqn: member_fqn.clone(),
                receiver_ty: *receiver_ty,
                explicit_arg_count: metadata.arg_count(),
                method_slot,
                interface_id: Some(interface_id),
                candidate_targets: contract.target_callables.clone(),
            }))
        }
        _ => Ok(None),
    }
}

fn dispatch_metadata(
    owner_fqn: &str,
    member_name: &str,
    member_fqn: &str,
    receiver_ty: TypeId,
) -> scoopc_lir::mir::DispatchMetadata {
    scoopc_lir::mir::DispatchMetadata {
        owner_fqn: owner_fqn.to_string(),
        member_name: member_name.to_string(),
        member_fqn: member_fqn.to_string(),
        member_decl_span: None,
        receiver_ty,
        stable_candidate_keys: Vec::new(),
        stable_template_key: None,
        generic_type_args: Vec::new(),
        generic_eff_args: Vec::new(),
    }
}

fn dynamic_invoke_carrier_contract(
    kind: &LateLoweredCallSiteMaterializedKind,
    carrier_source_ty: Option<TypeId>,
    dispatch: Option<LirDispatchKey>,
) -> Result<LirDynamicInvokeCarrierContract, EffectLoweringError> {
    let (carrier_kind, source_ty) = match kind {
        LateLoweredCallSiteMaterializedKind::Closure
        | LateLoweredCallSiteMaterializedKind::FunValue => (
            LirDynamicInvokeCarrierKind::ClosureObject,
            carrier_source_ty.ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
                detail: "callable dynamic invoke is missing carrier source type".to_string(),
            })?,
        ),
        LateLoweredCallSiteMaterializedKind::FunPtr => (
            LirDynamicInvokeCarrierKind::FunPtr,
            carrier_source_ty.ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
                detail: "FunPtr dynamic invoke is missing carrier source type".to_string(),
            })?,
        ),
        LateLoweredCallSiteMaterializedKind::Virtual { receiver_ty, .. } => (
            LirDynamicInvokeCarrierKind::VirtualReceiver,
            carrier_source_ty.unwrap_or(*receiver_ty),
        ),
        LateLoweredCallSiteMaterializedKind::Interface { receiver_ty, .. } => (
            LirDynamicInvokeCarrierKind::InterfaceReceiver,
            carrier_source_ty.unwrap_or(*receiver_ty),
        ),
        LateLoweredCallSiteMaterializedKind::Direct
        | LateLoweredCallSiteMaterializedKind::Resume => {
            return Err(EffectLoweringError::InvalidLirFactsContract {
                detail: format!("non-dynamic call kind {kind:?} cannot publish dynamic invoke"),
            });
        }
    };
    Ok(LirDynamicInvokeCarrierContract {
        kind: carrier_kind,
        source_ty: Some(source_ty),
        dispatch,
    })
}

fn is_dynamic_call_metadata(metadata: &LateLoweredCallSiteMaterializedMetadata) -> bool {
    matches!(
        metadata.kind(),
        LateLoweredCallSiteMaterializedKind::Closure
            | LateLoweredCallSiteMaterializedKind::FunValue
            | LateLoweredCallSiteMaterializedKind::FunPtr
            | LateLoweredCallSiteMaterializedKind::Virtual { .. }
            | LateLoweredCallSiteMaterializedKind::Interface { .. }
    )
}

fn call_site_contract(
    ctx: &LirFactsBuildContext<'_>,
    facts: &CallSiteEffectFacts,
) -> Result<LirCallSiteContract, EffectLoweringError> {
    let target_callables = target_callables(ctx, facts.target())?;
    let exact_callee = exact_callee_binding(ctx, facts.target_mode(), &target_callables)?;
    Ok(LirCallSiteContract {
        kind: call_site_kind(facts.kind()),
        target_mode: target_mode(facts.target_mode()),
        target_callables,
        exact_callee,
        callee_abi_kind: callable_abi_kind(facts.callee_abi_kind()),
        invoke_args_tuple_ty: facts.invoke_args_tuple_ty(),
        callee_step_schema: facts
            .callee_step_schema()
            .map(|schema| LirStepSchemaKey::new(schema.as_u32())),
        resolved_cases: facts
            .resolved_cases()
            .tags()
            .iter()
            .map(|case| LirCaseKey::new(case.as_u32()))
            .collect(),
        precision: effect_precision(facts.precision())?,
    })
}

fn exact_callee_binding(
    ctx: &LirFactsBuildContext<'_>,
    target_mode: CallTargetMode,
    target_callables: &[StableLirCallableKey],
) -> Result<Option<LirExactCalleeBinding>, EffectLoweringError> {
    if target_mode != CallTargetMode::KnownInstance {
        return Ok(None);
    }
    let [target_callable_key] = target_callables else {
        return invalid_lir_facts(format!(
            "known-instance call target must publish exactly one LIR callable key, got {}",
            target_callables.len()
        ));
    };
    let published_callable = published_callable_for_lir_key(ctx, target_callable_key)?;
    let root_fqn = published_callable
        .map(|callable| callable.root_fqn().to_string())
        .unwrap_or_else(|| target_callable_key.readable_path().to_string());
    let contracts = ctx.materialized.backend_contracts();
    let abi_symbol = if published_callable.is_some() {
        AbiMangler.fun_symbol(target_callable_key)
    } else {
        contracts
            .native_callable_funs
            .get(root_fqn.as_str())
            .map(|native| native.symbol.clone())
            .or_else(|| {
                contracts
                    .extern_funs
                    .get(root_fqn.as_str())
                    .map(|extern_fun| extern_fun.symbol.clone())
            })
            .ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
                detail: format!(
                    "known call target `{root_fqn}` has no published callable body or native/extern ABI symbol"
                ),
            })?
    };
    Ok(Some(LirExactCalleeBinding {
        target_callable_key: target_callable_key.clone(),
        signature_key: source_callable_signature_key(&root_fqn),
        abi_symbol,
        root_fqn,
    }))
}

fn published_callable_for_lir_key<'a>(
    ctx: &'a LirFactsBuildContext<'_>,
    key: &StableLirCallableKey,
) -> Result<Option<&'a LateLoweredCallable>, EffectLoweringError> {
    let canonical = key.canonical_text();
    for callable in ctx.lir.callables() {
        if published_callable_key(callable)?.canonical_text() == canonical {
            return Ok(Some(callable));
        }
    }
    Ok(None)
}

fn target_callables(
    ctx: &LirFactsBuildContext<'_>,
    target: &CallSiteTarget,
) -> Result<Vec<StableLirCallableKey>, EffectLoweringError> {
    match target {
        CallSiteTarget::KnownInstance(instance) => Ok(vec![target_callable_key(ctx, instance)?]),
        CallSiteTarget::CandidateSet(instances) => instances
            .iter()
            .map(|instance| target_callable_key(ctx, instance))
            .collect(),
        CallSiteTarget::DynamicFallback => Ok(Vec::new()),
    }
}

fn target_callable_key(
    ctx: &LirFactsBuildContext<'_>,
    instance: &InstanceKey,
) -> Result<StableLirCallableKey, EffectLoweringError> {
    let stable_key = ctx.lir.stable_instance_key(instance).ok_or_else(|| {
        EffectLoweringError::InvalidLirFactsContract {
            detail: format!(
                "call target `{}` has no published stable instance key",
                instance.template.fqn
            ),
        }
    })?;
    let stable_text = stable_key.canonical_text();
    if let Some(key) = ctx.callable_keys_by_stable_instance.get(&stable_text) {
        return Ok(key.clone());
    }
    Ok(declaration_lir_callable_key(stable_key))
}

fn declaration_lir_callable_key(
    stable_key: &scoopc_lir::stable_id::StableInstanceKey,
) -> StableLirCallableKey {
    StableLirCallableKey::new(
        canonical_record(
            "lir_callable",
            [stable_key.canonical_text(), "body#declaration".to_string()],
        ),
        stable_key.readable_path(),
    )
}

fn build_step_type_facts(lir: &LateLoweredProgram) -> BTreeMap<LirStepSchemaKey, LirStepTypeFacts> {
    lir.step_types()
        .iter()
        .map(|step| {
            let key = LirStepSchemaKey::new(step.step_schema().as_u32());
            (
                key,
                LirStepTypeFacts {
                    step_schema: key,
                    invoke_args_tuple_ty: step.invoke_args_tuple_ty(),
                    complete_ty: step.complete_ty(),
                    continuation_obj_ty: step.continuation_obj_ty(),
                    cases: step
                        .cases()
                        .iter()
                        .map(|case| LirStepCaseFacts {
                            case_tag: LirCaseKey::new(case.case_tag().as_u32()),
                            payload_tuple_ty: case.payload_tuple_ty(),
                            continuation_schema: LirContinuationSchemaKey::new(
                                case.continuation_schema().as_u32(),
                            ),
                        })
                        .collect(),
                },
            )
        })
        .collect()
}

fn build_resume_packing_facts(
    lir: &LateLoweredProgram,
) -> BTreeMap<LirResumePackingKey, LirResumePackingFacts> {
    lir.resume_packings()
        .iter()
        .map(|packing| {
            let key = LirResumePackingKey::new(packing.interface_id().as_u32());
            (
                key,
                LirResumePackingFacts {
                    interface_id: key,
                    effect_fqn: packing.effect_family().effect_fqn().to_string(),
                    effect_type_args: packing.effect_family().type_args().to_vec(),
                    return_step_schema: LirStepSchemaKey::new(
                        packing.return_step_schema().as_u32(),
                    ),
                    methods: packing
                        .methods()
                        .iter()
                        .map(|method| LirResumeMethodFacts {
                            case_tag: LirCaseKey::new(method.case_tag().as_u32()),
                            continuation_schema: LirContinuationSchemaKey::new(
                                method.continuation_schema().as_u32(),
                            ),
                            resume_tuple_ty: method.resume_tuple_ty(),
                            answer_ty: method.answer_ty(),
                            out_step_schema: LirStepSchemaKey::new(
                                method.out_step_schema().as_u32(),
                            ),
                            surface_ty: method.surface_ty(),
                        })
                        .collect(),
                },
            )
        })
        .collect()
}

fn build_continuation_object_facts(
    ctx: &LirFactsBuildContext<'_>,
) -> Result<BTreeMap<LirContinuationObjectKey, LirContinuationObjectFacts>, EffectLoweringError> {
    ctx.lir
        .continuation_objects()
        .iter()
        .map(|object| {
            let key = LirContinuationObjectKey::new(object.object_id().as_u32());
            Ok((
                key,
                LirContinuationObjectFacts {
                    object_id: key,
                    owner_body_version: body_version_key_for_owner(
                        ctx,
                        object.owner_version_key(),
                    )?,
                    continuation_obj_ty: object.continuation_obj_ty(),
                    implemented_packings: object
                        .implemented_packings()
                        .iter()
                        .map(|id| LirResumePackingKey::new(id.as_u32()))
                        .collect(),
                    surface_resumes: object
                        .surface_resumes()
                        .iter()
                        .map(|resume| LirContinuationResumeFacts {
                            case_tag: LirCaseKey::new(resume.case_tag().as_u32()),
                            continuation_schema: LirContinuationSchemaKey::new(
                                resume.continuation_schema().as_u32(),
                            ),
                            resume_tuple_ty: resume.resume_tuple_ty(),
                            answer_ty: resume.answer_ty(),
                            out_step_schema: LirStepSchemaKey::new(
                                resume.out_step_schema().as_u32(),
                            ),
                            surface_ty: resume.surface_ty(),
                            body: continuation_resume_body(resume.body()),
                        })
                        .collect(),
                    methods: object
                        .methods()
                        .iter()
                        .map(|method| LirContinuationMethodFacts {
                            packing_interface_id: LirResumePackingKey::new(
                                method.packing_interface_id().as_u32(),
                            ),
                            resume: LirContinuationResumeFacts {
                                case_tag: LirCaseKey::new(method.case_tag().as_u32()),
                                continuation_schema: LirContinuationSchemaKey::new(
                                    method.continuation_schema().as_u32(),
                                ),
                                resume_tuple_ty: method.resume_tuple_ty(),
                                answer_ty: method.answer_ty(),
                                out_step_schema: LirStepSchemaKey::new(
                                    method.out_step_schema().as_u32(),
                                ),
                                surface_ty: method.surface_ty(),
                                body: continuation_resume_body(method.body()),
                            },
                        })
                        .collect(),
                },
            ))
        })
        .collect()
}

fn build_surface_resume_dispatch_facts(
    lir: &LateLoweredProgram,
) -> BTreeMap<LirContinuationSchemaKey, LirSurfaceResumeDispatchFacts> {
    lir.surface_resume_dispatch_inventory()
        .iter()
        .map(|entry| {
            let contract = entry.contract();
            let key = LirContinuationSchemaKey::new(entry.continuation_schema().as_u32());
            (
                key,
                LirSurfaceResumeDispatchFacts {
                    continuation_schema: key,
                    resume_tuple_ty: contract.resume_tuple_ty(),
                    answer_ty: contract.answer_ty(),
                    out_step_schema: LirStepSchemaKey::new(contract.out_step_schema().as_u32()),
                    source_kind: surface_resume_source_kind(entry.source_kind()),
                    publication_count: entry.publications().len(),
                    wrapper_projection_count: entry.wrapper_projections().len(),
                },
            )
        })
        .collect()
}

fn body_version_key_for_owner(
    ctx: &LirFactsBuildContext<'_>,
    owner: &scoopc_lir::effect_lowered::ir::LateLoweredBodyVersionKey,
) -> Result<BodyVersionKey, EffectLoweringError> {
    let Some(stable) = ctx.lir.stable_instance_key(owner.surface_instance()) else {
        return Err(EffectLoweringError::InvalidLirFactsContract {
            detail: format!(
                "continuation owner `{}` has no published stable instance key",
                owner.surface_instance().template.fqn
            ),
        });
    };
    let stable_text = stable.canonical_text();
    let callable_key = ctx
        .callable_keys_by_stable_instance
        .get(&stable_text)
        .cloned()
        .ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
            detail: format!(
                "continuation owner `{}` ({stable_text}) has no published LIR callable key",
                owner.surface_instance().template.fqn
            ),
        })?;
    ctx.body_versions_by_key
        .get(&callable_key)
        .cloned()
        .ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
            detail: format!(
                "continuation owner `{}` has no published LIR body-version key",
                owner.surface_instance().template.fqn
            ),
        })
}

fn state_graph_facts(graph: &LateLoweredStateGraph) -> LirStateGraphFacts {
    LirStateGraphFacts {
        entry_state: LirStateKey::new(graph.entry_state().as_u32()),
        complete_state: LirStateKey::new(graph.complete_state().as_u32()),
        cleanup_state: graph
            .cleanup_state()
            .map(|state| LirStateKey::new(state.as_u32())),
        drop_state: graph
            .drop_state()
            .map(|state| LirStateKey::new(state.as_u32())),
        states: graph
            .states()
            .iter()
            .map(|state| LirStateKey::new(state.state_id().as_u32()))
            .collect(),
    }
}

fn frame_schema_facts(frame: &LateLoweredFrameSchema) -> LirFrameSchemaFacts {
    LirFrameSchemaFacts {
        slots: frame
            .slots()
            .iter()
            .map(|slot| LirFrameSlotFacts {
                slot_id: LirFrameSlotKey::new(slot.slot_id().as_u32()),
                ty: slot.ty(),
                kind: format!("{:?}", slot.kind()),
            })
            .collect(),
        resume_payload_bindings: frame
            .resume_payload_bindings()
            .iter()
            .map(|binding| LirResumePayloadBindingFacts {
                boundary_id: LirBoundaryKey::new(binding.boundary_id().as_u32()),
                resume_state: LirStateKey::new(binding.resume_state().as_u32()),
                consumer_local: LirLocalKey::new(binding.consumer_local().as_u32()),
                consumer_frame_slot: binding
                    .consumer_frame_slot()
                    .map(|slot| LirFrameSlotKey::new(slot.as_u32())),
            })
            .collect(),
        completion_payload_bindings: frame
            .completion_payload_bindings()
            .iter()
            .map(|binding| LirCompletionPayloadBindingFacts {
                return_state: LirStateKey::new(binding.return_state().as_u32()),
                complete_state: LirStateKey::new(binding.complete_state().as_u32()),
                payload_frame_slot: binding
                    .payload_frame_slot()
                    .map(|slot| LirFrameSlotKey::new(slot.as_u32())),
            })
            .collect(),
    }
}

fn boundary_map_facts(
    map: &LateLoweredBoundaryMap,
    dynamic_invokes: &HashMap<scoopc_lir::effect_lowered::ir::BoundaryId, LirDynamicInvokeKey>,
    dispatches: &HashMap<scoopc_lir::effect_lowered::ir::BoundaryId, LirDispatchKey>,
) -> LirBoundaryMapFacts {
    LirBoundaryMapFacts {
        boundaries: map
            .entries()
            .iter()
            .map(|boundary| {
                let boundary_id = boundary.boundary_id();
                let (source_kind, site_id) = boundary_source_facts(boundary.source());
                LirBoundaryFacts {
                    boundary_id: LirBoundaryKey::new(boundary_id.as_u32()),
                    source_kind,
                    site_id,
                    owner_state: LirStateKey::new(boundary.owner_state().as_u32()),
                    resume_state: LirStateKey::new(boundary.resume_state().as_u32()),
                    lowering_kind: boundary.lowering().map(boundary_lowering_kind),
                    dynamic_invoke: dynamic_invokes.get(&boundary_id).cloned(),
                    dispatch: dispatches.get(&boundary_id).cloned(),
                }
            })
            .collect(),
    }
}

fn resume_state_map_facts(map: &LateLoweredResumeStateMap) -> LirResumeStateMapFacts {
    LirResumeStateMapFacts {
        entries: map
            .entries()
            .iter()
            .map(|entry| LirResumeStateFacts {
                boundary_id: LirBoundaryKey::new(entry.boundary_id().as_u32()),
                state_id: LirStateKey::new(entry.state_id().as_u32()),
            })
            .collect(),
    }
}

fn dynamic_invoke_entry_facts(
    entry: &LateLoweredDynamicInvokeEntry,
) -> LirCallableDynamicInvokeEntryFacts {
    LirCallableDynamicInvokeEntryFacts {
        invoke_args_tuple_ty: entry.invoke_args_tuple_ty(),
        step_schema: LirStepSchemaKey::new(entry.step_schema().as_u32()),
        entry_state: LirStateKey::new(entry.entry_state().as_u32()),
        complete_state: LirStateKey::new(entry.complete_state().as_u32()),
    }
}

fn plain_slice_key(slice: LateLoweredPlainBodySlice) -> LirSourceSliceKey {
    LirSourceSliceKey {
        block_id: LirBodyBlockKey::new(slice.block_id().as_u32()),
        start_statement_index: slice.start_statement_index(),
        end_statement_index: slice.end_statement_index(),
        includes_terminator: slice.includes_terminator(),
    }
}

fn state_slice_key(slice: LateLoweredStateSlice) -> LirSourceSliceKey {
    LirSourceSliceKey {
        block_id: LirBodyBlockKey::new(slice.block_id().as_u32()),
        start_statement_index: slice.start_statement_index(),
        end_statement_index: slice.end_statement_index(),
        includes_terminator: slice.includes_terminator(),
    }
}

fn boundary_source_facts(source: LateLoweredBoundarySource) -> (String, Option<SiteId>) {
    match source {
        LateLoweredBoundarySource::Site { site_id, kind } => (format!("{:?}", kind), Some(site_id)),
        LateLoweredBoundarySource::RuntimeError { origin_site } => {
            ("RuntimeError".to_string(), Some(origin_site))
        }
    }
}

fn boundary_lowering_kind(lowering: &LateLoweredBoundaryLowering) -> String {
    match lowering {
        LateLoweredBoundaryLowering::Call(_) => "Call",
        LateLoweredBoundaryLowering::ClassCtor(_) => "ClassCtor",
        LateLoweredBoundaryLowering::Perform(_) => "Perform",
        LateLoweredBoundaryLowering::Resume(_) => "Resume",
        LateLoweredBoundaryLowering::RuntimeError(_) => "RuntimeError",
        LateLoweredBoundaryLowering::Handle(_) => "Handle",
    }
    .to_string()
}

fn call_site_kind(kind: CallSiteKind) -> LirCallSiteKind {
    match kind {
        CallSiteKind::Direct => LirCallSiteKind::Direct,
        CallSiteKind::Closure => LirCallSiteKind::Closure,
        CallSiteKind::FunValue => LirCallSiteKind::FunValue,
        CallSiteKind::FunPtr => LirCallSiteKind::FunPtr,
        CallSiteKind::Virtual => LirCallSiteKind::Virtual,
        CallSiteKind::Interface => LirCallSiteKind::Interface,
    }
}

fn target_mode(mode: CallTargetMode) -> LirCallTargetMode {
    match mode {
        CallTargetMode::KnownInstance => LirCallTargetMode::KnownInstance,
        CallTargetMode::CandidateSet => LirCallTargetMode::CandidateSet,
        CallTargetMode::DynamicFallback => LirCallTargetMode::DynamicFallback,
    }
}

fn callable_abi_kind(kind: CallableAbiKind) -> LirCallableAbiKind {
    match kind {
        CallableAbiKind::Plain => LirCallableAbiKind::Plain,
        CallableAbiKind::EffectStep => LirCallableAbiKind::EffectStep,
    }
}

fn effect_precision(precision: EffectPrecision) -> Result<LirEffectPrecision, EffectLoweringError> {
    match precision {
        EffectPrecision::Precise => Ok(LirEffectPrecision::Precise),
        EffectPrecision::Widened => Ok(LirEffectPrecision::Widened),
        EffectPrecision::SignatureFallback => Err(EffectLoweringError::InvalidLirFactsContract {
            detail: "P5 received obsolete signature-fallback call precision".to_string(),
        }),
    }
}

fn continuation_resume_body(body: LateLoweredContinuationResumeBody) -> LirContinuationResumeBody {
    match body {
        LateLoweredContinuationResumeBody::ResumeCapturedState { repeated_resume } => match repeated_resume {
            scoopc_lir::effect_lowered::ir::LateLoweredOneShotPolicy::OrdinaryRuntimeErrorOutward => {
                LirContinuationResumeBody::OneShotRuntimeErrorPublication
            }
        },
        LateLoweredContinuationResumeBody::Unreachable => LirContinuationResumeBody::Unreachable,
    }
}

fn surface_resume_source_kind(
    kind: LateLoweredSurfaceResumeDispatchSourceKind,
) -> LirSurfaceResumeDispatchSourceKind {
    match kind {
        LateLoweredSurfaceResumeDispatchSourceKind::ContinuationObjectMethod => {
            LirSurfaceResumeDispatchSourceKind::ContinuationObjectMethod
        }
        LateLoweredSurfaceResumeDispatchSourceKind::ResumeBoundaryOnly => {
            LirSurfaceResumeDispatchSourceKind::ResumeBoundaryOnly
        }
        LateLoweredSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly => {
            LirSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly
        }
        LateLoweredSurfaceResumeDispatchSourceKind::OwnerTrampolineMixed => {
            LirSurfaceResumeDispatchSourceKind::OwnerTrampolineMixed
        }
        LateLoweredSurfaceResumeDispatchSourceKind::Unreachable => {
            LirSurfaceResumeDispatchSourceKind::Unreachable
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use scoopc_ids::AbiMangler;
    use scoopc_lir::effect_lowered::ir::{LateLoweredPlainCallable, LateLoweredProgram};
    use scoopc_lir::mir::{InstanceKey, TemplateKey};
    use scoopc_lir::span::Span;
    use scoopc_lir::stable_id::{
        StableConeKey, StableDefKey, StableDefNamespace, StableInstanceKey, StableTemplateKey,
    };
    use scoopc_lir::ty::{EffectRow, TypeStore};

    fn test_callable(root_fqn: &str, source_path: &str, span: Span) -> LateLoweredCallable {
        let template = TemplateKey {
            fqn: root_fqn.to_string(),
            source_path: PathBuf::from(source_path),
            decl_span: span,
        };
        let instance = InstanceKey {
            template: template.clone(),
            type_args: Vec::new(),
            eff_args: Vec::new(),
        };
        let stable_template = StableTemplateKey::new(StableDefKey::new(
            StableConeKey::new("fixture", "0.0.0"),
            StableDefNamespace::Fun,
            root_fqn,
            "fun",
            None,
        ));
        let stable_instance =
            StableInstanceKey::from_canonical_args(stable_template, Vec::new(), Vec::new());
        let body_version_key = scoopc_lir::effect_lowered::ir::LateLoweredBodyVersionKey::new(
            instance,
            EffectRow::pure(),
            ImplPlan::NoOutward,
            false,
        );
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let plain = LateLoweredPlainCallable::new(
            builtins.unit,
            Vec::new(),
            builtins.unit,
            Vec::new(),
            Vec::new(),
            None,
        );
        LateLoweredCallable::new_plain(
            root_fqn.to_string(),
            stable_instance,
            body_version_key,
            Vec::new(),
            plain,
        )
    }

    #[test]
    fn stable_lir_callable_key_ignores_program_local_callable_order() {
        let mut types = TypeStore::new();
        types.intern_builtins();

        let dep = test_callable(
            "lib.dependencyValue",
            "/workspace/deps/lib/api.scoop",
            Span::new(10, 30),
        );
        let consumer = test_callable("app.main", "/workspace/src/main.scoop", Span::new(40, 60));
        let dep_only =
            LateLoweredProgram::new(Vec::new(), Vec::new(), Vec::new(), vec![dep.clone()]);
        let consumer_then_dep =
            LateLoweredProgram::new(Vec::new(), Vec::new(), Vec::new(), vec![consumer, dep]);

        let dep_only_key = stable_lir_callable_key(
            &dep_only,
            &types,
            dep_only.callable("lib.dependencyValue").unwrap(),
        )
        .unwrap();
        let consumer_then_dep_key = stable_lir_callable_key(
            &consumer_then_dep,
            &types,
            consumer_then_dep.callable("lib.dependencyValue").unwrap(),
        )
        .unwrap();

        assert_eq!(dep_only_key, consumer_then_dep_key);
        assert!(dep_only_key.as_str().contains("body#h"));
        assert_eq!(dep_only_key.readable_path(), "lib.dependencyValue");
        assert_eq!(
            AbiMangler.fun_symbol(&dep_only_key),
            AbiMangler.fun_symbol(&consumer_then_dep_key)
        );
    }
}
