//! Stable textual dump entry for LIR facts.

use std::fmt::Write as _;

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
        "  summary: callables={} step_types={} resume_packings={} continuation_objects={} surface_resume_dispatches={}",
        facts.summary.callable_count,
        facts.summary.step_type_count,
        facts.summary.resume_packing_count,
        facts.summary.continuation_object_count,
        facts.summary.surface_resume_dispatch_count,
    )
    .expect("writing to String cannot fail");
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
    }
    write!(&mut out, "}}").expect("writing to String cannot fail");

    out
}
