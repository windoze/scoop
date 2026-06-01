//! Stable textual dump entry for MIR facts.

use std::fmt::Write as _;

use scoopc_ids::StableCanonicalKey as _;

use crate::MirFacts;
use crate::effects::{CallSiteTarget, CallSiteTargetSource, MirEffectEventKind};
use crate::provenance::{
    CallableValueProvenance, CallableValueProvenanceSource, ResultProvenance,
    ResultProvenanceSource,
};
use crate::roots::{MirRootDetail, MirRootFact};

/// Render a compact, stable summary of the MIR fact groups.
pub fn dump_mir_facts(facts: &MirFacts) -> String {
    let mut out = String::new();

    writeln!(&mut out, "mir_facts {{").expect("writing to String cannot fail");
    writeln!(
        &mut out,
        "  roots: callable_bodies={}, initializers={}, extern_globals={}, metadata_roots={}",
        facts.roots.callable_bodies.len(),
        facts.roots.initializers.len(),
        facts.roots.extern_globals.len(),
        facts.roots.metadata_roots.len()
    )
    .expect("writing to String cannot fail");
    dump_root_group(&mut out, "callable_bodies", &facts.roots.callable_bodies);
    dump_root_group(&mut out, "initializers", &facts.roots.initializers);
    dump_root_group(&mut out, "extern_globals", &facts.roots.extern_globals);
    dump_root_group(&mut out, "metadata_roots", &facts.roots.metadata_roots);
    writeln!(
        &mut out,
        "  snapshots: canonical={}, snapshots={}",
        facts
            .snapshots
            .canonical
            .as_ref()
            .map(|key| key.canonical_text())
            .unwrap_or_else(|| "<none>".to_string()),
        facts.snapshots.snapshots.len()
    )
    .expect("writing to String cannot fail");
    for snapshot in &facts.snapshots.snapshots {
        writeln!(
            &mut out,
            "    - key={} opt_level={} bodies={} revision={}",
            snapshot.key.canonical_text(),
            snapshot.opt_level.as_str(),
            snapshot.body_count,
            snapshot.revision,
        )
        .expect("writing to String cannot fail");
        for fqn in &snapshot.canonical_body_fqns {
            writeln!(&mut out, "      body={fqn}").expect("writing to String cannot fail");
        }
    }
    writeln!(
        &mut out,
        "  families: instances={}, callable_families={}",
        facts.families.instances.len(),
        facts.families.callable_families.len()
    )
    .expect("writing to String cannot fail");
    for instance in &facts.families.instances {
        writeln!(
            &mut out,
            "    - instance={} callable={}{}",
            instance.artifact.canonical_text(),
            instance.callable.as_str(),
            instance
                .body
                .as_ref()
                .map(|body| format!(" body={}", body.fqn))
                .unwrap_or_default(),
        )
        .expect("writing to String cannot fail");
    }
    if !facts.effects.is_empty() {
        writeln!(
            &mut out,
            "  effects: callable_instances={}, sites={}, events={}, block_regions={}, call_targets={}, call_surfaces={}",
            facts.effects.callable_instances.len(),
            facts.effects.site_inventory.len(),
            facts.effects.effect_events.len(),
            facts.effects.block_regions.len(),
            facts.effects.call_site_targets.len(),
            facts.effects.call_site_surface_effects.len(),
        )
        .expect("writing to String cannot fail");
        for instance in &facts.effects.callable_instances {
            writeln!(
                &mut out,
                "    - instance_effect={} callable={} published={} step={}",
                instance.instance.canonical_text(),
                instance.callable.as_str(),
                instance.published_surface_row.canonical_text(),
                instance.step_effect_row.canonical_text(),
            )
            .expect("writing to String cannot fail");
        }
        for site in &facts.effects.site_inventory {
            writeln!(
                &mut out,
                "    - site={} body={} kind={} block=bb{}{}",
                site.site_id.as_u32(),
                site.body.fqn,
                site.kind.label(),
                site.block.as_u32(),
                site.result_local
                    .map(|local| format!(" result=local{local}"))
                    .unwrap_or_default(),
            )
            .expect("writing to String cannot fail");
        }
        for event in &facts.effects.effect_events {
            writeln!(
                &mut out,
                "    - event={} body={} kind={} row={} cleanup={}",
                event.site_id.as_u32(),
                event.body.fqn,
                event_kind_label(&event.kind),
                event.effect_row.canonical_text(),
                event.cleanup,
            )
            .expect("writing to String cannot fail");
        }
        for target in &facts.effects.call_site_targets {
            writeln!(
                &mut out,
                "    - call_target={} body={} kind={} target={}",
                target.site_id.as_u32(),
                target.body.fqn,
                target.call_kind.label(),
                call_target_label(&target.target),
            )
            .expect("writing to String cannot fail");
        }
    }
    if !facts.provenance.is_empty() {
        writeln!(
            &mut out,
            "  provenance: callable_values={}, results={}",
            facts.provenance.callable_values.len(),
            facts.provenance.results.len(),
        )
        .expect("writing to String cannot fail");
        for result in &facts.provenance.results {
            writeln!(
                &mut out,
                "    - result={} callable={} provenance={} overridden={}",
                result.instance.canonical_text(),
                result.callable.as_str(),
                result_provenance_label(&result.provenance),
                result.summary_overridden,
            )
            .expect("writing to String cannot fail");
        }
        for value in &facts.provenance.callable_values {
            writeln!(
                &mut out,
                "    - callable_value body={} local={} provenance={}",
                value.body.fqn,
                value.local,
                callable_value_provenance_label(&value.provenance),
            )
            .expect("writing to String cannot fail");
        }
    }
    if !facts.boundary.is_empty() {
        writeln!(
            &mut out,
            "  boundary: source_contracts={}",
            facts.boundary.source_contracts.len(),
        )
        .expect("writing to String cannot fail");
    }
    if !facts.backend.is_empty() {
        writeln!(
            &mut out,
            "  backend: source_signatures={}, enum_layouts={}, class_inits={}, vtables={}, interfaces={}, itables={}, extern_funs={}, native_callable_funs={}, global_inits={}",
            facts.backend.source_signatures.len(),
            facts.backend.enum_layouts.len(),
            facts.backend.class_inits.len(),
            facts.backend.vtables.len(),
            facts.backend.interfaces.len(),
            facts.backend.itables.len(),
            facts.backend.extern_funs.len(),
            facts.backend.native_callable_funs.len(),
            facts.backend.global_inits.len(),
        )
        .expect("writing to String cannot fail");
    }
    writeln!(
        &mut out,
        "  pass_artifacts: revisions={}, callable_body_overrides={}, summaries={}, escape_facts={}",
        facts.pass_artifacts.revisions.len(),
        facts.pass_artifacts.callable_body_overrides.len(),
        facts.pass_artifacts.summary_revisions.len(),
        facts.pass_artifacts.escape_facts.len()
    )
    .expect("writing to String cannot fail");
    for revision in &facts.pass_artifacts.revisions {
        writeln!(
            &mut out,
            "    - revision={} pass={} index={}",
            revision.key.canonical_text(),
            revision.pass_name,
            revision.revision,
        )
        .expect("writing to String cannot fail");
    }
    writeln!(
        &mut out,
        "  pass_pipeline: runs={}",
        facts.pass_pipeline.runs.len()
    )
    .expect("writing to String cannot fail");
    for run in &facts.pass_pipeline.runs {
        writeln!(
            &mut out,
            "    - pass={} enabled={} input={} output={} changed_bodies={} changed_summaries={} escape_facts={}",
            run.pass.as_str(),
            run.enabled,
            run.input_revision
                .as_ref()
                .map(|key| key.canonical_text())
                .unwrap_or_else(|| "<none>".to_string()),
            run.output_revision
                .as_ref()
                .map(|key| key.canonical_text())
                .unwrap_or_else(|| "<none>".to_string()),
            run.changed_bodies,
            run.changed_summaries,
            run.produced_escape_facts,
        )
        .expect("writing to String cannot fail");
    }
    write!(&mut out, "}}").expect("writing to String cannot fail");

    out
}

fn event_kind_label(kind: &MirEffectEventKind) -> String {
    match kind {
        MirEffectEventKind::Call { call_kind } => format!("call:{}", call_kind.label()),
        MirEffectEventKind::ClassCtor { source_fqn } => format!("class_ctor:{source_fqn}"),
        MirEffectEventKind::HiddenInitializer { source_fqn } => {
            format!("hidden_initializer:{source_fqn}")
        }
        MirEffectEventKind::Perform { op } => format!("perform:{}", op.op_fqn),
        MirEffectEventKind::Resume { .. } => "resume".to_string(),
        MirEffectEventKind::Handle { arms, .. } => format!(
            "handle:{}",
            arms.iter()
                .map(|arm| arm.op_fqn.as_str())
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

fn call_target_label(target: &CallSiteTarget) -> String {
    match target {
        CallSiteTarget::KnownInstance { key } => format!("known:{}", key.as_str()),
        CallSiteTarget::CandidateSet { keys } => format!(
            "candidates:{}",
            keys.iter()
                .map(|key| key.as_str())
                .collect::<Vec<_>>()
                .join(",")
        ),
        CallSiteTarget::DirectFunction { fqn } => format!("direct:{fqn}"),
        CallSiteTarget::KnownClosure { fn_ptr } => format!("closure:{fn_ptr}"),
        CallSiteTarget::Param { index } => format!("param:{index}"),
        CallSiteTarget::Join {
            sources,
            requires_dynamic_fallback,
        } => format!(
            "join:{} fallback={}",
            sources
                .iter()
                .map(call_target_source_label)
                .collect::<Vec<_>>()
                .join("|"),
            requires_dynamic_fallback,
        ),
        CallSiteTarget::DynamicFallback { reason } => format!("dynamic_fallback:{reason:?}"),
        CallSiteTarget::Dynamic => "dynamic".to_string(),
    }
}

fn call_target_source_label(source: &CallSiteTargetSource) -> String {
    match source {
        CallSiteTargetSource::KnownInstance { key } => format!("known:{}", key.as_str()),
        CallSiteTargetSource::DirectFunction { fqn } => format!("direct:{fqn}"),
        CallSiteTargetSource::KnownClosure { fn_ptr } => format!("closure:{fn_ptr}"),
        CallSiteTargetSource::Param { index } => format!("param:{index}"),
    }
}

fn callable_value_provenance_label(provenance: &CallableValueProvenance) -> String {
    match provenance {
        CallableValueProvenance::DirectFunction { fqn } => format!("direct:{fqn}"),
        CallableValueProvenance::KnownInstance { key } => format!("known:{}", key.as_str()),
        CallableValueProvenance::KnownClosure { fn_ptr } => format!("closure:{fn_ptr}"),
        CallableValueProvenance::Param { index } => format!("param:{index}"),
        CallableValueProvenance::Join { sources } => format!(
            "join:{}",
            sources
                .iter()
                .map(callable_value_source_label)
                .collect::<Vec<_>>()
                .join("|")
        ),
        CallableValueProvenance::Unknown => "unknown".to_string(),
    }
}

fn callable_value_source_label(source: &CallableValueProvenanceSource) -> String {
    match source {
        CallableValueProvenanceSource::DirectFunction { fqn } => format!("direct:{fqn}"),
        CallableValueProvenanceSource::KnownInstance { key } => format!("known:{}", key.as_str()),
        CallableValueProvenanceSource::KnownClosure { fn_ptr } => format!("closure:{fn_ptr}"),
        CallableValueProvenanceSource::Param { index } => format!("param:{index}"),
    }
}

fn result_provenance_label(provenance: &ResultProvenance) -> String {
    match provenance {
        ResultProvenance::Unit => "unit".to_string(),
        ResultProvenance::Param { index } => format!("param:{index}"),
        ResultProvenance::DirectFunction { fqn } => format!("direct:{fqn}"),
        ResultProvenance::KnownClosure { fn_ptr } => format!("closure:{fn_ptr}"),
        ResultProvenance::TopLevelValue { fqn } => format!("top_level:{fqn}"),
        ResultProvenance::PerformResult { op_fqn } => format!("perform:{op_fqn}"),
        ResultProvenance::Join { sources } => format!(
            "join:{}",
            sources
                .iter()
                .map(result_provenance_source_label)
                .collect::<Vec<_>>()
                .join("|")
        ),
        ResultProvenance::Unknown => "unknown".to_string(),
    }
}

fn result_provenance_source_label(source: &ResultProvenanceSource) -> String {
    match source {
        ResultProvenanceSource::Param { index } => format!("param:{index}"),
        ResultProvenanceSource::DirectFunction { fqn } => format!("direct:{fqn}"),
        ResultProvenanceSource::KnownClosure { fn_ptr } => format!("closure:{fn_ptr}"),
        ResultProvenanceSource::TopLevelValue { fqn } => format!("top_level:{fqn}"),
        ResultProvenanceSource::PerformResult { op_fqn } => format!("perform:{op_fqn}"),
    }
}

fn dump_root_group(out: &mut String, label: &str, roots: &[MirRootFact]) {
    if roots.is_empty() {
        return;
    }

    writeln!(out, "  {label} {{").expect("writing to String cannot fail");
    for root in roots {
        writeln!(
            out,
            "    - item={} fqn={} kind={}{}{}{}",
            root.item.index,
            root.fqn,
            root.kind.label(),
            source_path_suffix(root),
            span_suffix(root),
            detail_suffix(root)
        )
        .expect("writing to String cannot fail");
    }
    writeln!(out, "  }}").expect("writing to String cannot fail");
}

fn source_path_suffix(root: &MirRootFact) -> String {
    root.source_path
        .as_ref()
        .map(|path| format!(" source_path={path}"))
        .unwrap_or_default()
}

fn span_suffix(root: &MirRootFact) -> String {
    root.span
        .map(|span| format!(" span={span:?}"))
        .unwrap_or_default()
}

fn detail_suffix(root: &MirRootFact) -> String {
    match &root.detail {
        MirRootDetail::CallableBody => String::new(),
        MirRootDetail::Initializer {
            kind,
            has_initializer,
            dependency_count,
        } => format!(
            " initializer_kind={} has_initializer={} dependencies={}",
            kind.label(),
            has_initializer,
            dependency_count
        ),
        MirRootDetail::ExternGlobal {
            storage,
            mutable,
            symbol,
            initializer_absent,
            unsafe_required,
        } => format!(
            " storage={} mutable={} symbol={} initializer_absent={} unsafe_required={}",
            storage.label(),
            mutable,
            symbol,
            initializer_absent,
            unsafe_required
        ),
        MirRootDetail::Metadata { kind } => format!(" metadata_kind={}", kind.label()),
    }
}
