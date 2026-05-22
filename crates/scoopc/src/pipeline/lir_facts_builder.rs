//! Production adapter from `LateLoweredProgram` to the data-only LIR facts crate.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use scoopc_ids::{BodyVersionKey, StableCanonicalKey, StableLirCallableKey, StableSymbolKey};
use scoopc_lir_facts::*;
use scoopc_mir_facts::MirFacts;
use scoopc_mir_facts::roots::{
    MirGlobalStorageKind, MirInitializerDependencyKind, MirInitializerRootKind, MirRootDetail,
    MirRootFact,
};

use crate::effect_facts::{
    CallSiteEffectFacts, CallSiteKind, CallSiteTarget, CallTargetMode, CallableAbiKind,
    EffectPrecision, MaterializedEffectFacts, SiteEffectFacts,
};
use crate::effect_lowered::EffectLoweringError;
use crate::effect_lowered::ir::{
    BoundarySiteKind, LateLoweredBoundaryLowering, LateLoweredBoundaryMap,
    LateLoweredBoundarySource, LateLoweredCallable, LateLoweredContinuationResumeBody,
    LateLoweredDynamicInvokeEntry, LateLoweredEffectStepCallable, LateLoweredFrameSchema,
    LateLoweredPlainBodySlice, LateLoweredPlainCallSite, LateLoweredPlainCallable,
    LateLoweredProgram, LateLoweredResumeStateMap, LateLoweredStateGraph, LateLoweredStateSlice,
    LateLoweredSurfaceResumeDispatchSourceKind,
};
use crate::mir::{
    Body, CallKind as MirCallKind, FunDecl, InstanceKey, Item, MaterializedMir,
    Operand as MirOperand, Rvalue as MirRvalue, SiteId, StatementKind as MirStatementKind,
};
use crate::opt::OptLevel;
use crate::ty::{TypeId, TypeStore};

struct LirFactsBuildContext<'a> {
    lir: &'a LateLoweredProgram,
    mir_facts: &'a MirFacts,
    materialized: &'a MaterializedMir,
    effect_facts: &'a MaterializedEffectFacts,
    callable_keys_by_root: HashMap<String, StableLirCallableKey>,
    body_versions_by_key: HashMap<StableLirCallableKey, BodyVersionKey>,
}

#[derive(Debug, Clone)]
struct MaterializedCallSite {
    kind: MirCallKind,
    arg_count: usize,
    carrier_source_ty: Option<TypeId>,
}

#[derive(Debug, Clone)]
struct MaterializedCallableSignature {
    source_kind: LirCallableSourceKind,
    param_tys: Vec<TypeId>,
    return_ty: TypeId,
    closure_carrier_arg_tys: Vec<TypeId>,
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
    let callable_keys_by_root = callable_key_index(lir);
    let body_versions_by_key = body_version_index(lir, &callable_keys_by_root)?;
    let ctx = LirFactsBuildContext {
        lir,
        mir_facts,
        materialized,
        effect_facts,
        callable_keys_by_root,
        body_versions_by_key,
    };

    let mut dynamic_invokes = BTreeMap::new();
    let mut dispatches = BTreeMap::new();
    let callables = build_callable_facts(&ctx, &mut dynamic_invokes, &mut dispatches)?;
    let groups = LirFactGroups {
        global_init: build_global_init_facts(&ctx)?,
        physical_layout: build_physical_layout_facts(&ctx)?,
        type_context: build_type_context_facts(ctx.materialized, ctx.effect_facts),
        callables,
        step_types: build_step_type_facts(lir),
        dynamic_invokes,
        dispatches,
        resume_packings: build_resume_packing_facts(lir),
        continuation_objects: build_continuation_object_facts(&ctx),
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

fn callable_key_index(lir: &LateLoweredProgram) -> HashMap<String, StableLirCallableKey> {
    lir.callables()
        .iter()
        .enumerate()
        .map(|(index, callable)| {
            (
                callable.root_fqn().to_string(),
                stable_lir_callable_key(index, callable),
            )
        })
        .collect()
}

fn body_version_index(
    lir: &LateLoweredProgram,
    callable_keys_by_root: &HashMap<String, StableLirCallableKey>,
) -> Result<HashMap<StableLirCallableKey, BodyVersionKey>, EffectLoweringError> {
    let mut out = HashMap::new();
    for callable in lir.callables() {
        let key = callable_key_for_root(callable_keys_by_root, callable.root_fqn())?;
        out.insert(key.clone(), body_version_facts(callable, key).key);
    }
    Ok(out)
}

fn stable_lir_callable_key(index: usize, callable: &LateLoweredCallable) -> StableLirCallableKey {
    let stable_instance_key = callable.stable_instance_key();
    StableLirCallableKey::new(
        format!(
            "lir_callable({},body#{index})",
            stable_instance_key.canonical_text()
        ),
        stable_instance_key.readable_path(),
    )
}

fn callable_key_for_root<'a>(
    callable_keys_by_root: &'a HashMap<String, StableLirCallableKey>,
    root_fqn: &str,
) -> Result<&'a StableLirCallableKey, EffectLoweringError> {
    callable_keys_by_root.get(root_fqn).ok_or_else(|| {
        EffectLoweringError::InvalidLirFactsContract {
            detail: format!("missing stable LIR callable key for `{root_fqn}`"),
        }
    })
}

fn body_version_facts(
    callable: &LateLoweredCallable,
    key: &StableLirCallableKey,
) -> LirBodyVersionFacts {
    let role = format!(
        "{:?}:impl={:?}:reentry={}",
        callable.call_abi_kind(),
        callable.impl_plan(),
        callable.needs_reentry()
    );
    LirBodyVersionFacts {
        key: BodyVersionKey::new(key, role, 0),
        impl_plan: format!("{:?}", callable.impl_plan()),
        needs_reentry: callable.needs_reentry(),
        allowed_effect_terms: callable.allowed_row().terms.clone(),
    }
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
) -> Result<LirPhysicalLayoutFacts, EffectLoweringError> {
    let contracts = ctx.materialized.backend_contracts();
    let mut facts = LirPhysicalLayoutFacts::default();

    for (layout_key, class) in &contracts.class_inits {
        insert_class_layout_fact(&mut facts, layout_key.as_str(), class);
    }
    for (fqn, layout) in &contracts.enum_layouts {
        facts.enums.insert(fqn.clone(), enum_layout_facts(layout));
    }
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
    Ok(facts)
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
                        ty: field.ty,
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn build_callable_symbol_facts(
    ctx: &LirFactsBuildContext<'_>,
) -> Result<BTreeMap<StableLirCallableKey, LirCallableSymbolFacts>, EffectLoweringError> {
    let contracts = ctx.materialized.backend_contracts();
    let mut out = BTreeMap::new();
    for callable in ctx.lir.callables() {
        let key = callable_key_for_root(&ctx.callable_keys_by_root, callable.root_fqn())?.clone();
        let signature =
            lookup_materialized_callable_signature(ctx.materialized, callable.root_fqn())?;
        let native = contracts
            .native_callable_funs
            .get(callable.root_fqn())
            .map(|native| LirNativeCallableSignatureFacts {
                symbol: native.symbol.clone(),
                calling_convention: native.calling_convention.clone(),
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
                param_tys: signature.param_tys.clone(),
                return_ty: signature.return_ty,
            });
        out.insert(
            key.clone(),
            LirCallableSymbolFacts {
                callable: key,
                root_fqn: callable.root_fqn().to_string(),
                stable_instance_key: callable.stable_instance_key().canonical_text(),
                exported_symbol: ctx
                    .materialized
                    .instance_exported_fun_symbol(callable.instance_key()),
                kind: callable_symbol_kind(callable, native.as_ref(), extern_.as_ref()),
                abi_kind: callable_abi_kind(callable.call_abi_kind()),
                param_tys: signature.param_tys,
                return_ty: signature.return_ty,
                native,
                extern_,
            },
        );
    }
    Ok(out)
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
            decision: LirTypeStableWireFormatDecision::Deferred,
            owner: "P8 per-cone build artifact serialization".to_string(),
            reason: "TypeId is still an in-process TypeStore identity; portable serialization needs a canonical text key or import remap format.".to_string(),
            non_blocking_reason: "P7-T04 only needs a single in-process LIR/base type context owner; persisted per-cone artifacts are not emitted yet.".to_string(),
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
        let key = callable_key_for_root(&ctx.callable_keys_by_root, callable.root_fqn())?.clone();
        let contract = match callable.abi() {
            crate::effect_lowered::ir::LateLoweredCallableAbi::Plain(plain) => {
                LirCallableContract::Plain(Box::new(build_plain_callable_facts(
                    ctx,
                    callable,
                    &key,
                    plain,
                    dynamic_invokes,
                    dispatches,
                )?))
            }
            crate::effect_lowered::ir::LateLoweredCallableAbi::EffectStep(effect) => {
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
        let signature =
            lookup_materialized_callable_signature(ctx.materialized, callable.root_fqn())?;
        out.insert(
            key.clone(),
            LirCallableFacts {
                root_fqn: callable.root_fqn().to_string(),
                stable_instance_key: callable.stable_instance_key().canonical_text(),
                source_kind: signature.source_kind,
                param_tys: signature.param_tys,
                return_ty: signature.return_ty,
                body_version: body_version_facts(callable, &key),
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
    let signature = lookup_materialized_callable_signature(ctx.materialized, callable.root_fqn())?;
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
    let materialized = lookup_materialized_call_site(ctx, callable.root_fqn(), site.site_id())?;
    let contract = call_site_contract(ctx, site.facts());
    let dispatch = publish_dispatch_contract(
        ctx,
        owner_key,
        site.site_id(),
        &materialized.kind,
        materialized.arg_count,
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
        &materialized,
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
    step_schema: crate::effect_facts::StepSchemaId,
    state_graph: &LateLoweredStateGraph,
    frame_schema: &LateLoweredFrameSchema,
    boundary_map: &LateLoweredBoundaryMap,
    resume_state_map: &LateLoweredResumeStateMap,
    classifications: &[crate::effect_lowered::ir::LateLoweredSourceStatementClassification],
    continuation_object: crate::effect_lowered::ir::ContinuationObjectId,
    resume_packings: &[crate::effect_lowered::ir::ResumeInterfaceId],
    dynamic_invokes: &mut BTreeMap<LirDynamicInvokeKey, LirDynamicInvokeContract>,
    dispatches: &mut BTreeMap<LirDispatchKey, LirDispatchContract>,
) -> Result<LirControlBodyFacts, EffectLoweringError> {
    let mut boundary_dynamic_invokes = HashMap::new();
    let mut boundary_dispatches = HashMap::new();
    let mut boundary_call_sites = BTreeSet::new();

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
        boundary_call_sites.insert(site_id);
        let materialized = lookup_materialized_call_site(ctx, callable.root_fqn(), site_id)?;
        let contract = call_site_contract(ctx, lowering.facts());
        let dispatch = publish_dispatch_contract(
            ctx,
            owner_key,
            site_id,
            &materialized.kind,
            materialized.arg_count,
            &contract,
            dispatches,
        )?;
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
            &materialized,
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
        state_graph,
        &boundary_call_sites,
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
    owner_step_schema: crate::effect_facts::StepSchemaId,
    state_graph: &LateLoweredStateGraph,
    boundary_call_sites: &BTreeSet<SiteId>,
    dynamic_invokes: &mut BTreeMap<LirDynamicInvokeKey, LirDynamicInvokeContract>,
    dispatches: &mut BTreeMap<LirDispatchKey, LirDispatchContract>,
) -> Result<(), EffectLoweringError> {
    let body = lookup_materialized_callable_body(ctx.materialized, callable.root_fqn())?;
    let body_facts = ctx
        .effect_facts
        .body(callable.instance_key())
        .ok_or_else(|| EffectLoweringError::MissingBodyFacts {
            root_fqn: callable.root_fqn().to_string(),
        })?;
    for state in state_graph.states() {
        for source_slice in state.source_slices() {
            let Some(block) = body.blocks.get(source_slice.block_id().as_u32() as usize) else {
                return Err(EffectLoweringError::InvalidLirFactsContract {
                    detail: format!(
                        "callable `{}` source slice references missing block bb{}",
                        callable.root_fqn(),
                        source_slice.block_id().as_u32()
                    ),
                });
            };
            let start = source_slice.start_statement_index() as usize;
            let end = source_slice.end_statement_index() as usize;
            if start > end || end > block.stmts.len() {
                return Err(EffectLoweringError::InvalidLirFactsContract {
                    detail: format!(
                        "callable `{}` source slice [{}..{}) is outside block bb{}",
                        callable.root_fqn(),
                        start,
                        end,
                        source_slice.block_id().as_u32()
                    ),
                });
            }
            for (offset, statement) in block.stmts[start..end].iter().enumerate() {
                let MirStatementKind::Assign {
                    value:
                        MirRvalue::Call {
                            site_id,
                            kind,
                            args,
                            ..
                        },
                    ..
                } = &statement.kind
                else {
                    continue;
                };
                if boundary_call_sites.contains(site_id) || !is_dynamic_call_kind(kind) {
                    continue;
                }
                let site = body_facts.site(*site_id).ok_or_else(|| {
                    EffectLoweringError::MissingSiteFacts {
                        root_fqn: callable.root_fqn().to_string(),
                        site_id: site_id.as_u32(),
                    }
                })?;
                let SiteEffectFacts::Call(call_facts) = site else {
                    return Err(EffectLoweringError::UnexpectedSiteFactsKind {
                        root_fqn: callable.root_fqn().to_string(),
                        site_id: site_id.as_u32(),
                        expected: "Call",
                        actual: crate::effect_lowered::materialize::site_facts_kind(site),
                    });
                };
                let contract = call_site_contract(ctx, call_facts);
                let materialized = MaterializedCallSite {
                    kind: kind.clone(),
                    arg_count: args.len(),
                    carrier_source_ty: dynamic_call_carrier_source_ty(body, kind),
                };
                let dispatch = publish_dispatch_contract(
                    ctx,
                    owner_key,
                    *site_id,
                    kind,
                    args.len(),
                    &contract,
                    dispatches,
                )?;
                publish_dynamic_invoke_contract(
                    ctx,
                    owner_key,
                    Some(LirStepSchemaKey::new(owner_step_schema.as_u32())),
                    *site_id,
                    LirDynamicInvokeSource::ControlSourceSlice {
                        source_slice: state_slice_key(*source_slice),
                        statement_index: source_slice.start_statement_index() + offset as u32,
                    },
                    contract,
                    &materialized,
                    dispatch,
                    dynamic_invokes,
                )?;
            }
        }
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
    materialized: &MaterializedCallSite,
    dispatch: Option<LirDispatchKey>,
    dynamic_invokes: &mut BTreeMap<LirDynamicInvokeKey, LirDynamicInvokeContract>,
) -> Result<Option<LirDynamicInvokeKey>, EffectLoweringError> {
    if contract.callee_step_schema.is_none() || !is_dynamic_call_kind(&materialized.kind) {
        return Ok(None);
    }
    let key = LirDynamicInvokeKey {
        owner_callable: owner_key.clone(),
        site_id,
    };
    if dynamic_invokes.contains_key(&key) {
        return Ok(Some(key));
    }
    let carrier = dynamic_invoke_carrier_contract(
        &materialized.kind,
        materialized.carrier_source_ty,
        dispatch,
    )?;
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
            arg_count: materialized.arg_count,
            target_body_versions,
        },
    );
    Ok(Some(key))
}

fn publish_dispatch_contract(
    ctx: &LirFactsBuildContext<'_>,
    owner_key: &StableLirCallableKey,
    site_id: SiteId,
    kind: &MirCallKind,
    arg_count: usize,
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
    let Some(dispatch) = dispatch_contract(ctx, owner_key, site_id, kind, arg_count, contract)?
    else {
        return Ok(None);
    };
    dispatches.insert(key.clone(), dispatch);
    Ok(Some(key))
}

fn dispatch_contract(
    ctx: &LirFactsBuildContext<'_>,
    owner_key: &StableLirCallableKey,
    site_id: SiteId,
    kind: &MirCallKind,
    arg_count: usize,
    contract: &LirCallSiteContract,
) -> Result<Option<LirDispatchContract>, EffectLoweringError> {
    let facts = ctx.materialized.dispatch_devirtualization_facts();
    match kind {
        MirCallKind::Virtual { dispatch, .. } => {
            let method_slot = facts
                .virtual_method_slot(dispatch, arg_count)
                .ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
                    detail: format!(
                        "virtual dispatch {}.{} at site{} has no unique vtable slot",
                        dispatch.owner_fqn,
                        dispatch.member_name,
                        site_id.as_u32()
                    ),
                })?;
            Ok(Some(LirDispatchContract {
                owner_callable: owner_key.clone(),
                site_id,
                kind: LirCallSiteKind::Virtual,
                owner_fqn: dispatch.owner_fqn.clone(),
                member_name: dispatch.member_name.clone(),
                member_fqn: dispatch.member_fqn.clone(),
                receiver_ty: dispatch.receiver_ty,
                explicit_arg_count: arg_count,
                method_slot,
                interface_id: None,
                candidate_targets: contract.target_callables.clone(),
            }))
        }
        MirCallKind::Interface { dispatch, .. } => {
            let (interface_id, method_slot) = facts
                .interface_method_slot(dispatch, arg_count)
                .ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
                    detail: format!(
                        "interface dispatch {} at site{} has no unique itable slot",
                        dispatch.member_fqn,
                        site_id.as_u32()
                    ),
                })?;
            Ok(Some(LirDispatchContract {
                owner_callable: owner_key.clone(),
                site_id,
                kind: LirCallSiteKind::Interface,
                owner_fqn: dispatch.owner_fqn.clone(),
                member_name: dispatch.member_name.clone(),
                member_fqn: dispatch.member_fqn.clone(),
                receiver_ty: dispatch.receiver_ty,
                explicit_arg_count: arg_count,
                method_slot,
                interface_id: Some(interface_id),
                candidate_targets: contract.target_callables.clone(),
            }))
        }
        _ => Ok(None),
    }
}

fn dynamic_invoke_carrier_contract(
    kind: &MirCallKind,
    carrier_source_ty: Option<TypeId>,
    dispatch: Option<LirDispatchKey>,
) -> Result<LirDynamicInvokeCarrierContract, EffectLoweringError> {
    let (carrier_kind, source_ty) = match kind {
        MirCallKind::Closure { .. } | MirCallKind::FunValue { .. } => (
            LirDynamicInvokeCarrierKind::ClosureObject,
            carrier_source_ty.ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
                detail: "callable dynamic invoke is missing carrier source type".to_string(),
            })?,
        ),
        MirCallKind::FunPtr { .. } => (
            LirDynamicInvokeCarrierKind::FunPtr,
            carrier_source_ty.ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
                detail: "FunPtr dynamic invoke is missing carrier source type".to_string(),
            })?,
        ),
        MirCallKind::Virtual { dispatch, .. } => (
            LirDynamicInvokeCarrierKind::VirtualReceiver,
            carrier_source_ty.unwrap_or(dispatch.receiver_ty),
        ),
        MirCallKind::Interface { dispatch, .. } => (
            LirDynamicInvokeCarrierKind::InterfaceReceiver,
            carrier_source_ty.unwrap_or(dispatch.receiver_ty),
        ),
        MirCallKind::Direct { .. } | MirCallKind::Resume { .. } => {
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

fn call_site_contract(
    ctx: &LirFactsBuildContext<'_>,
    facts: &CallSiteEffectFacts,
) -> LirCallSiteContract {
    LirCallSiteContract {
        kind: call_site_kind(facts.kind()),
        target_mode: target_mode(facts.target_mode()),
        target_callables: target_callables(ctx, facts.target()),
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
        precision: effect_precision(facts.precision()),
    }
}

fn target_callables(
    ctx: &LirFactsBuildContext<'_>,
    target: &CallSiteTarget,
) -> Vec<StableLirCallableKey> {
    match target {
        CallSiteTarget::KnownInstance(instance) => vec![target_callable_key(ctx, instance)],
        CallSiteTarget::CandidateSet(instances) => instances
            .iter()
            .map(|instance| target_callable_key(ctx, instance))
            .collect(),
        CallSiteTarget::DynamicFallback => Vec::new(),
    }
}

fn target_callable_key(
    ctx: &LirFactsBuildContext<'_>,
    instance: &InstanceKey,
) -> StableLirCallableKey {
    if let Some(key) = ctx.callable_keys_by_root.get(&instance.template.fqn) {
        return key.clone();
    }
    if let Some(stable_key) = ctx.lir.stable_instance_key(instance) {
        return StableLirCallableKey::new(
            format!("lir_callable({})", stable_key.canonical_text()),
            stable_key.readable_path(),
        );
    }
    StableLirCallableKey::new(
        format!("lir_callable(unpublished({}))", instance.template.fqn),
        instance.template.fqn.clone(),
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
) -> BTreeMap<LirContinuationObjectKey, LirContinuationObjectFacts> {
    ctx.lir
        .continuation_objects()
        .iter()
        .map(|object| {
            let key = LirContinuationObjectKey::new(object.object_id().as_u32());
            (
                key,
                LirContinuationObjectFacts {
                    object_id: key,
                    owner_body_version: body_version_key_for_owner(ctx, object.owner_version_key()),
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
            )
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
    owner: &crate::effect_lowered::ir::LateLoweredBodyVersionKey,
) -> BodyVersionKey {
    let Some(stable) = ctx.lir.stable_instance_key(owner.surface_instance()) else {
        return BodyVersionKey::from_parts("missing-owner", "unknown", 0);
    };
    let callable_key = ctx
        .callable_keys_by_root
        .get(stable.readable_path())
        .cloned()
        .unwrap_or_else(|| {
            StableLirCallableKey::new(
                format!("lir_callable({})", stable.canonical_text()),
                stable.readable_path(),
            )
        });
    ctx.body_versions_by_key
        .get(&callable_key)
        .cloned()
        .unwrap_or_else(|| BodyVersionKey::new(&callable_key, "owner", 0))
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
    dynamic_invokes: &HashMap<crate::effect_lowered::ir::BoundaryId, LirDynamicInvokeKey>,
    dispatches: &HashMap<crate::effect_lowered::ir::BoundaryId, LirDispatchKey>,
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

fn lookup_materialized_callable_body<'a>(
    materialized: &'a MaterializedMir,
    root_fqn: &str,
) -> Result<&'a Body, EffectLoweringError> {
    let fun = lookup_materialized_callable_fun(materialized, root_fqn)?;
    fun.body
        .as_ref()
        .ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
            detail: format!("callable `{root_fqn}` has no materialized body"),
        })
}

fn lookup_materialized_callable_signature(
    materialized: &MaterializedMir,
    root_fqn: &str,
) -> Result<MaterializedCallableSignature, EffectLoweringError> {
    let fun = lookup_materialized_callable_fun(materialized, root_fqn)?;
    let param_tys = fun.params.iter().map(|param| param.ty).collect::<Vec<_>>();
    let skip_closure_env = usize::from(fun.name.starts_with("$lambda"));
    let closure_carrier_arg_tys = param_tys.iter().copied().skip(skip_closure_env).collect();
    Ok(MaterializedCallableSignature {
        source_kind: callable_source_kind(materialized, root_fqn),
        param_tys,
        return_ty: fun.return_ty,
        closure_carrier_arg_tys,
    })
}

fn callable_source_kind(materialized: &MaterializedMir, root_fqn: &str) -> LirCallableSourceKind {
    let base_fqn = callable_source_base_fqn(root_fqn);
    if base_fqn.contains(".$lambda") || callable_owner_is_nominal_or_object(materialized, base_fqn)
    {
        return LirCallableSourceKind::MemberOrSynthetic;
    }
    LirCallableSourceKind::TopLevel
}

fn callable_source_base_fqn(root_fqn: &str) -> &str {
    let base = root_fqn
        .rsplit_once("::<")
        .map(|(base, _)| base)
        .unwrap_or(root_fqn);
    base.split_once("$overload$")
        .map(|(base, _)| base)
        .unwrap_or(base)
}

fn callable_owner_is_nominal_or_object(materialized: &MaterializedMir, root_fqn: &str) -> bool {
    let Some((owner_fqn, _name)) = root_fqn.rsplit_once('.') else {
        return false;
    };
    materialized.file.items.iter().any(|item| match item {
        Item::Metadata(crate::mir::MetadataRoot::Nominal(metadata)) => metadata.fqn == owner_fqn,
        Item::Metadata(crate::mir::MetadataRoot::Object(metadata)) => metadata.fqn == owner_fqn,
        _ => false,
    })
}

fn lookup_materialized_callable_fun<'a>(
    materialized: &'a MaterializedMir,
    root_fqn: &str,
) -> Result<&'a FunDecl, EffectLoweringError> {
    let pass_view = materialized.pass_view();
    pass_view
        .callable(root_fqn)
        .or_else(|| {
            materialized.file.items.iter().find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == root_fqn => Some(fun),
                _ => None,
            })
        })
        .or_else(|| {
            materialized
                .caller_side_pass_candidate_bodies()
                .iter()
                .find(|fun| fun.fqn == root_fqn)
        })
        .ok_or_else(|| EffectLoweringError::InvalidLirFactsContract {
            detail: format!("missing materialized signature for callable `{root_fqn}`"),
        })
}

fn lookup_materialized_call_site(
    ctx: &LirFactsBuildContext<'_>,
    root_fqn: &str,
    site_id: SiteId,
) -> Result<MaterializedCallSite, EffectLoweringError> {
    let body = lookup_materialized_callable_body(ctx.materialized, root_fqn)?;
    for block in &body.blocks {
        for statement in &block.stmts {
            let MirStatementKind::Assign {
                value:
                    MirRvalue::Call {
                        site_id: stmt_site_id,
                        kind,
                        args,
                        ..
                    },
                ..
            } = &statement.kind
            else {
                continue;
            };
            if *stmt_site_id == site_id {
                return Ok(MaterializedCallSite {
                    kind: kind.clone(),
                    arg_count: args.len(),
                    carrier_source_ty: dynamic_call_carrier_source_ty(body, kind),
                });
            }
        }
    }
    Err(EffectLoweringError::InvalidLirFactsContract {
        detail: format!(
            "callable `{root_fqn}` site{} has no materialized call metadata",
            site_id.as_u32()
        ),
    })
}

fn dynamic_call_carrier_source_ty(body: &Body, kind: &MirCallKind) -> Option<TypeId> {
    match kind {
        MirCallKind::Closure { callee, .. }
        | MirCallKind::FunValue { callee }
        | MirCallKind::FunPtr { callee } => materialized_operand_source_ty(body, callee),
        MirCallKind::Virtual { receiver, dispatch }
        | MirCallKind::Interface { receiver, dispatch } => {
            materialized_operand_source_ty(body, receiver).or(Some(dispatch.receiver_ty))
        }
        MirCallKind::Direct { .. } | MirCallKind::Resume { .. } => None,
    }
}

fn materialized_operand_source_ty(body: &Body, operand: &MirOperand) -> Option<TypeId> {
    match operand {
        MirOperand::Local(local) => body.locals.get(local.as_u32() as usize).map(|decl| decl.ty),
        MirOperand::Const(_) => None,
    }
}

fn is_dynamic_call_kind(kind: &MirCallKind) -> bool {
    matches!(
        kind,
        MirCallKind::Closure { .. }
            | MirCallKind::FunValue { .. }
            | MirCallKind::FunPtr { .. }
            | MirCallKind::Virtual { .. }
            | MirCallKind::Interface { .. }
    )
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

fn effect_precision(precision: EffectPrecision) -> LirEffectPrecision {
    match precision {
        EffectPrecision::Precise => LirEffectPrecision::Precise,
        EffectPrecision::Widened => LirEffectPrecision::Widened,
        EffectPrecision::SignatureFallback => LirEffectPrecision::SignatureFallback,
    }
}

fn continuation_resume_body(body: LateLoweredContinuationResumeBody) -> LirContinuationResumeBody {
    match body {
        LateLoweredContinuationResumeBody::ResumeCapturedState { repeated_resume } => match repeated_resume {
            crate::effect_lowered::ir::LateLoweredOneShotPolicy::OrdinaryRuntimeErrorOutward => {
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
