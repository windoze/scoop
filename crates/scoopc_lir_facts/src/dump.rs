//! Stable textual dump entry for LIR facts.

use std::fmt::Write as _;

use scoopc_ids::StableCanonicalKey as _;

use crate::LirFacts;

/// Render a compact, stable summary of LIR fact groups.
pub fn dump_lir_facts(facts: &LirFacts) -> String {
    let mut out = String::new();

    writeln!(&mut out, "lir_facts {{").expect("writing to String cannot fail");
    writeln!(
        &mut out,
        "  opt_level: O{}",
        facts.summary.opt_level.as_str()
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut out,
        "  summary: opt_revision={} callables={} step_types={} dynamic_invokes={} dispatches={} resume_packings={} continuation_objects={} surface_resume_dispatches={} layout_classes={} layout_enums={} layout_interfaces={} layout_class_itables={} layout_callable_symbols={}",
        facts.summary.opt_revision,
        facts.summary.callable_count,
        facts.summary.step_type_count,
        facts.dynamic_invokes.len(),
        facts.dispatches.len(),
        facts.summary.resume_packing_count,
        facts.summary.continuation_object_count,
        facts.summary.surface_resume_dispatch_count,
        facts.summary.layout_class_count,
        facts.summary.layout_enum_count,
        facts.summary.layout_interface_count,
        facts.summary.layout_class_itable_count,
        facts.summary.layout_callable_symbol_count,
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut out,
        "  opt_pipeline: revision={} preserve_published_resume_shells={} passes={}",
        facts.opt_pipeline.revision,
        facts.opt_pipeline.preserve_published_resume_shells,
        facts.opt_pipeline.passes.len(),
    )
    .expect("writing to String cannot fail");
    for pass in &facts.opt_pipeline.passes {
        writeln!(
            &mut out,
            "    - pass={} status={:?} changed={}",
            pass.kind.stable_name(),
            pass.status,
            pass.changed,
        )
        .expect("writing to String cannot fail");
    }
    dump_global_init_facts(&mut out, facts);
    dump_physical_layout_facts(&mut out, facts);
    dump_type_context_facts(&mut out, facts);
    writeln!(&mut out, "  callables={}", facts.callables.len())
        .expect("writing to String cannot fail");
    for (id, callable) in &facts.callables {
        writeln!(
            &mut out,
            "    - callable={} kind={:?} control_body={} id={:?} key={}",
            callable.root_fqn(),
            callable.kind(),
            callable.has_control_body(),
            id,
            callable.body_version.key.owner_canonical_text(),
        )
        .expect("writing to String cannot fail");
        match &callable.contract {
            crate::LirCallableContract::Plain(plain) => {
                writeln!(
                    &mut out,
                    "      plain_abi: function_ty=t{} params={} return=t{} body_slices={} call_sites={}",
                    plain.function_ty.as_u32(),
                    plain.param_tys.len(),
                    plain.return_ty.as_u32(),
                    plain.body_slices.len(),
                    plain.call_sites.len(),
                )
                .expect("writing to String cannot fail");
                if let Some(control) = &plain.local_effect_control {
                    writeln!(
                        &mut out,
                        "      plain_local_effect_control: step#{} states={} boundaries={} continuation_object=cont_obj#{}",
                        control.step_schema.as_u32(),
                        control.state_graph.states.len(),
                        control.boundary_map.boundaries.len(),
                        control.continuation_object.as_u32(),
                    )
                    .expect("writing to String cannot fail");
                }
            }
            crate::LirCallableContract::EffectStep(effect) => {
                writeln!(
                    &mut out,
                    "      effect_step_abi: step#{} invoke_args=t{} entry=st{} complete=st{} states={} boundaries={} continuation_object=cont_obj#{}",
                    effect.step_schema.as_u32(),
                    effect.dynamic_invoke_entry.invoke_args_tuple_ty.as_u32(),
                    effect.dynamic_invoke_entry.entry_state.as_u32(),
                    effect.dynamic_invoke_entry.complete_state.as_u32(),
                    effect.control_body.state_graph.states.len(),
                    effect.control_body.boundary_map.boundaries.len(),
                    effect.control_body.continuation_object.as_u32(),
                )
                .expect("writing to String cannot fail");
            }
        }
    }
    writeln!(&mut out, "  step_types={}", facts.step_types.len())
        .expect("writing to String cannot fail");
    for (key, step) in &facts.step_types {
        writeln!(
            &mut out,
            "    - step#{} invoke_args=t{} complete=t{} continuation_obj=t{} cases={}",
            key.as_u32(),
            step.invoke_args_tuple_ty.as_u32(),
            step.complete_ty.as_u32(),
            step.continuation_obj_ty.as_u32(),
            step.cases.len(),
        )
        .expect("writing to String cannot fail");
    }
    writeln!(
        &mut out,
        "  dynamic_invoke_contracts={}",
        facts.dynamic_invokes.len()
    )
    .expect("writing to String cannot fail");
    for (key, contract) in &facts.dynamic_invokes {
        writeln!(
            &mut out,
            "    - owner={} site={} kind={:?} target_mode={:?} callee_step={:?} carrier={:?} arg_count={} targets={}",
            key.owner_callable.readable_path(),
            key.site_id.as_u32(),
            contract.call.kind,
            contract.call.target_mode,
            contract.call.callee_step_schema.map(|schema| schema.as_u32()),
            contract.carrier.kind,
            contract.arg_count,
            contract.target_body_versions.len(),
        )
        .expect("writing to String cannot fail");
    }
    writeln!(
        &mut out,
        "  source_call_site_contracts={}",
        facts.source_call_sites.len()
    )
    .expect("writing to String cannot fail");
    for (key, site) in &facts.source_call_sites {
        writeln!(
            &mut out,
            "    - owner={} site={} kind={:?} target_mode={:?} semantic_root={} named_entry={} generic_args={} exact_callee={}",
            key.owner_callable.readable_path(),
            key.site_id.as_u32(),
            site.contract.kind,
            site.contract.target_mode,
            site.semantic_root_fqn.as_deref().unwrap_or("<none>"),
            site.named_entry_name.as_deref().unwrap_or("<none>"),
            site.generic_type_args.len(),
            site.contract
                .exact_callee
                .as_ref()
                .map(|exact| exact.root_fqn.as_str())
                .unwrap_or("<none>"),
        )
        .expect("writing to String cannot fail");
    }
    writeln!(
        &mut out,
        "  class_ctor_call_site_contracts={}",
        facts.class_ctor_call_sites.len()
    )
    .expect("writing to String cannot fail");
    for (key, site) in &facts.class_ctor_call_sites {
        writeln!(
            &mut out,
            "    - owner={} site={} class={} target={} arg_slots={}",
            key.owner_callable.readable_path(),
            key.site_id.as_u32(),
            site.class_fqn,
            site.target_init.as_str(),
            site.arg_mapping.len(),
        )
        .expect("writing to String cannot fail");
    }
    writeln!(
        &mut out,
        "  reflection_call_site_contracts={}",
        facts.reflection_call_sites.len()
    )
    .expect("writing to String cannot fail");
    for (key, site) in &facts.reflection_call_sites {
        writeln!(
            &mut out,
            "    - owner={} site={} intrinsic={} type_args={}",
            key.owner_callable.readable_path(),
            key.site_id.as_u32(),
            site.intrinsic_name,
            site.type_args.len(),
        )
        .expect("writing to String cannot fail");
    }
    writeln!(&mut out, "  dispatch_contracts={}", facts.dispatches.len())
        .expect("writing to String cannot fail");
    for (key, dispatch) in &facts.dispatches {
        writeln!(
            &mut out,
            "    - owner={} site={} kind={:?} dispatch_owner={} member={} slot={} interface_id={:?} candidates={}",
            key.owner_callable.readable_path(),
            key.site_id.as_u32(),
            dispatch.kind,
            dispatch.owner_fqn,
            dispatch.member_name,
            dispatch.method_slot,
            dispatch.interface_id,
            dispatch.candidate_targets.len(),
        )
        .expect("writing to String cannot fail");
    }
    writeln!(
        &mut out,
        "  resume_packing_contracts={}",
        facts.resume_packings.len()
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut out,
        "  continuation_object_contracts={}",
        facts.continuation_objects.len()
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut out,
        "  surface_resume_dispatch_contracts={}",
        facts.surface_resume_dispatches.len()
    )
    .expect("writing to String cannot fail");
    write!(&mut out, "}}").expect("writing to String cannot fail");

    out
}

fn dump_global_init_facts(out: &mut String, facts: &LirFacts) {
    if facts.global_init.is_empty() {
        return;
    }

    writeln!(
        out,
        "  global_init: roots={} object_once={} top_level_eager_inits={} cone_init_routines={} final_entry_routines={}",
        facts.global_init.roots.len(),
        facts.global_init.object_once.len(),
        facts.global_init.top_level_eager_inits.len(),
        facts.global_init.cone_init_routines.len(),
        facts.global_init.final_entry_order.routines.len(),
    )
    .expect("writing to String cannot fail");
    for (key, root) in &facts.global_init.roots {
        writeln!(
            out,
            "    - root={} kind={} cone={} source_order={} storage={} has_initializer={} deps={} ty={}{}",
            key.as_str(),
            root.kind.stable_name(),
            root.cone.canonical_text(),
            root.source_cone_order,
            root.storage
                .map(|storage| storage.stable_name())
                .unwrap_or("<none>"),
            root.has_initializer,
            root.dependencies.len(),
            root.ty
                .map(|ty| format!("t{}", ty.as_u32()))
                .unwrap_or_else(|| "<none>".to_string()),
            root.extern_global
                .as_ref()
                .map(|extern_global| format!(
                    " extern_symbol={} mutable={} initializer_absent={} unsafe_required={}",
                    extern_global.symbol,
                    extern_global.mutable,
                    extern_global.initializer_absent,
                    extern_global.unsafe_required,
                ))
                .unwrap_or_default(),
        )
        .expect("writing to String cannot fail");
        if let Some(body) = &root.initializer_body {
            writeln!(
                out,
                "      initializer_body kind={} source={} span={}..{} items={}",
                body.kind.stable_name(),
                body.source_path,
                body.source_span_start,
                body.source_span_end,
                body.body_item_count,
            )
            .expect("writing to String cannot fail");
        }
        for dependency in &root.dependencies {
            writeln!(
                out,
                "      depends_on={} kind={}",
                dependency.target.as_str(),
                dependency.kind.stable_name(),
            )
            .expect("writing to String cannot fail");
        }
    }
    for (key, object_once) in &facts.global_init.object_once {
        writeln!(
            out,
            "    - object_once={} has_initializer={}",
            key.as_str(),
            object_once.has_initializer,
        )
        .expect("writing to String cannot fail");
    }
    for (key, eager) in &facts.global_init.top_level_eager_inits {
        writeln!(
            out,
            "    - top_level_eager_init={} storage={} has_initializer={}",
            key.as_str(),
            eager
                .storage
                .map(|storage| storage.stable_name())
                .unwrap_or("<none>"),
            eager.has_initializer,
        )
        .expect("writing to String cannot fail");
    }
    for (key, routine) in &facts.global_init.cone_init_routines {
        let roots = routine
            .roots
            .iter()
            .map(|root| root.as_str())
            .collect::<Vec<_>>()
            .join(",");
        writeln!(
            out,
            "    - cone_init_routine=r{} cone={} source_order={} roots=[{}]",
            key.as_u32(),
            routine.cone.canonical_text(),
            routine.source_cone_order,
            roots,
        )
        .expect("writing to String cannot fail");
    }
    let routines = facts
        .global_init
        .final_entry_order
        .routines
        .iter()
        .map(|routine| format!("r{}", routine.as_u32()))
        .collect::<Vec<_>>()
        .join(",");
    writeln!(out, "    - final_entry_init_order=[{}]", routines)
        .expect("writing to String cannot fail");
}

fn dump_physical_layout_facts(out: &mut String, facts: &LirFacts) {
    if facts.physical_layout.is_empty() {
        return;
    }

    writeln!(
        out,
        "  physical_layout: classes={} enums={} vtables={} interfaces={} class_itables={} callable_symbols={}",
        facts.physical_layout.classes.len(),
        facts.physical_layout.enums.len(),
        facts.physical_layout.class_vtables.len(),
        facts.physical_layout.interfaces.len(),
        facts.physical_layout.class_itables.len(),
        facts.physical_layout.callable_symbols.len(),
    )
    .expect("writing to String cannot fail");
    for (key, class) in &facts.physical_layout.classes {
        writeln!(
            out,
            "    - class={} layout_key={} super={} fields={}",
            key,
            class.layout_key,
            class.super_class_fqn.as_deref().unwrap_or("<none>"),
            class.fields.len(),
        )
        .expect("writing to String cannot fail");
    }
    for (key, enum_layout) in &facts.physical_layout.enums {
        writeln!(
            out,
            "    - enum={} repr={} variants={}",
            key,
            enum_layout.repr.stable_name(),
            enum_layout.variants.len(),
        )
        .expect("writing to String cannot fail");
    }
    for (key, slots) in &facts.physical_layout.class_vtables {
        writeln!(out, "    - class_vtable={} slots={}", key, slots.len())
            .expect("writing to String cannot fail");
    }
    for (key, interface) in &facts.physical_layout.interfaces {
        writeln!(
            out,
            "    - interface={} id={} methods={} supers={}",
            key,
            interface.interface_id,
            interface.method_slots.len(),
            interface.super_interfaces.len(),
        )
        .expect("writing to String cannot fail");
    }
    for (key, itable) in &facts.physical_layout.class_itables {
        writeln!(
            out,
            "    - class_itable={} entries={}",
            key,
            itable.entries.len()
        )
        .expect("writing to String cannot fail");
    }
    for (id, symbol) in &facts.physical_layout.callable_symbols {
        writeln!(
            out,
            "    - callable_symbol={:?} key={} root={} kind={} abi={:?} exported={}",
            id,
            symbol.callable.as_str(),
            symbol.root_fqn,
            symbol.kind.stable_name(),
            symbol.abi_kind,
            symbol.exported_symbol.as_deref().unwrap_or("<none>"),
        )
        .expect("writing to String cannot fail");
    }
}

fn dump_type_context_facts(out: &mut String, facts: &LirFacts) {
    let ctx = &facts.type_context;
    if ctx.primary_fingerprint.is_empty()
        && ctx.materialized_fingerprint.is_empty()
        && ctx.effect_facts_fingerprint.is_empty()
    {
        return;
    }
    writeln!(
        out,
        "  type_context: owner={} bridge={} remapped_types={} wire_format={} wire_owner={}",
        ctx.owner.stable_name(),
        ctx.bridge_mode.stable_name(),
        ctx.remapped_type_count,
        ctx.stable_wire_format.decision.stable_name(),
        ctx.stable_wire_format.owner,
    )
    .expect("writing to String cannot fail");
}
