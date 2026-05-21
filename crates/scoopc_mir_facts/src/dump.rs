//! Stable textual dump entry for MIR facts.

use std::fmt::Write as _;

use scoopc_ids::StableCanonicalKey as _;

use crate::MirFacts;

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
