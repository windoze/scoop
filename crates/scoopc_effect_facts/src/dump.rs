//! Stable textual dump entry for effect facts.

use std::fmt::Write as _;

use crate::EffectFacts;

/// Render a compact, stable summary of effect fact groups.
pub fn dump_effect_facts(facts: &EffectFacts) -> String {
    let mut out = String::new();

    writeln!(&mut out, "effect_facts {{").expect("writing to String cannot fail");
    writeln!(
        &mut out,
        "  snapshot_binding: surface={:?} instances={} canonical_bodies={}",
        facts.snapshot_binding.query_surface(),
        facts.snapshot_binding.instance_count(),
        facts.snapshot_binding.canonical_body_fqns().len()
    )
    .expect("writing to String cannot fail");
    for fqn in facts.snapshot_binding.canonical_body_fqns() {
        writeln!(&mut out, "    - body={fqn}").expect("writing to String cannot fail");
    }
    writeln!(
        &mut out,
        "  schemas: steps={} continuations={}",
        facts.step_schemas.len(),
        facts.continuation_schemas.len()
    )
    .expect("writing to String cannot fail");
    for (schema_id, schema) in &facts.step_schemas {
        writeln!(
            &mut out,
            "    - step=s{} cases={}",
            schema_id.as_u32(),
            schema.cases().len()
        )
        .expect("writing to String cannot fail");
    }
    writeln!(&mut out, "  callables={}", facts.callables.len())
        .expect("writing to String cannot fail");
    for (key, callable) in &facts.callables {
        writeln!(
            &mut out,
            "    - callable={} abi={:?} step={} outward_cases={}",
            key.readable_path(),
            callable.call_abi_kind(),
            callable
                .body_step_schema()
                .map(|schema| format!("s{}", schema.as_u32()))
                .unwrap_or_else(|| "<none>".to_string()),
            callable.resolved_outward_cases().tags().len()
        )
        .expect("writing to String cannot fail");
    }
    writeln!(&mut out, "  bodies={}", facts.bodies.len()).expect("writing to String cannot fail");
    for (key, body) in &facts.bodies {
        writeln!(
            &mut out,
            "    - body={} blocks={} sites={} local_control_step={}",
            key.readable_path(),
            body.blocks().len(),
            body.sites().len(),
            body.local_control_step_schema()
                .map(|schema| format!("s{}", schema.as_u32()))
                .unwrap_or_else(|| "<none>".to_string())
        )
        .expect("writing to String cannot fail");
    }
    write!(&mut out, "}}").expect("writing to String cannot fail");

    out
}
