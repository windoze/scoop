//! Stable textual dump entry for MIR facts.

use std::fmt::Write as _;

use scoopc_ids::StableCanonicalKey as _;

use crate::MirFacts;
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
    writeln!(
        &mut out,
        "  families: instances={}, callable_families={}",
        facts.families.instances.len(),
        facts.families.callable_families.len()
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut out,
        "  pass_artifacts: revisions={}, callable_body_overrides={}, summaries={}, escape_facts={}",
        facts.pass_artifacts.revisions.len(),
        facts.pass_artifacts.callable_body_overrides.len(),
        facts.pass_artifacts.summary_revisions.len(),
        facts.pass_artifacts.escape_facts.len()
    )
    .expect("writing to String cannot fail");
    write!(
        &mut out,
        "  pass_pipeline: runs={}\n}}",
        facts.pass_pipeline.runs.len()
    )
    .expect("writing to String cannot fail");

    out
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
