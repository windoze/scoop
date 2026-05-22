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
        "  summary: opt_revision={} callables={} step_types={} dynamic_invokes={} dispatches={} resume_packings={} continuation_objects={} surface_resume_dispatches={}",
        facts.summary.opt_revision,
        facts.summary.callable_count,
        facts.summary.step_type_count,
        facts.dynamic_invokes.len(),
        facts.dispatches.len(),
        facts.summary.resume_packing_count,
        facts.summary.continuation_object_count,
        facts.summary.surface_resume_dispatch_count,
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
    writeln!(&mut out, "  callables={}", facts.callables.len())
        .expect("writing to String cannot fail");
    for (key, callable) in &facts.callables {
        writeln!(
            &mut out,
            "    - callable={} kind={:?} control_body={} key={}",
            callable.root_fqn(),
            callable.kind(),
            callable.has_control_body(),
            key.as_str(),
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
            "    - root={} kind={} cone={} storage={} has_initializer={} deps={} ty={}{}",
            key.as_str(),
            root.kind.stable_name(),
            root.cone.canonical_text(),
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
            "    - cone_init_routine=r{} cone={} roots=[{}]",
            key.as_u32(),
            routine.cone.canonical_text(),
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
