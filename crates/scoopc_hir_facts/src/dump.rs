//! Stable textual dump entry for HIR facts.

use std::fmt::Write as _;

use crate::HirFacts;

/// Render a compact, stable summary of the HIR fact groups.
pub fn dump_hir_facts(facts: &HirFacts) -> String {
    let mut out = String::new();

    writeln!(&mut out, "hir_facts {{").expect("writing to String cannot fail");
    writeln!(
        &mut out,
        "  declarations: nominals={}, callables={}, fields={}, vtables={}, itables={}",
        facts.declarations.nominals.len(),
        facts.declarations.callables.len(),
        facts.declarations.fields.len(),
        facts.declarations.dispatch.vtables.len(),
        facts.declarations.dispatch.interface_tables.len()
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut out,
        "  source_sites: calls={}, args={}, assigns={}, updates={}, effects={}, resumes={}, patterns={}",
        facts.source_sites.call_sites.len(),
        facts.source_sites.argument_bindings.len(),
        facts.source_sites.assignments.len(),
        facts.source_sites.with_updates.len(),
        facts.source_sites.effect_sites.len(),
        facts.source_sites.continuation_resumes.len(),
        facts.source_sites.pattern_bindings.len()
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut out,
        "  globals: roots={}, object_inits={}, class_inits={}",
        facts.globals.roots.len(),
        facts.globals.object_initializers.len(),
        facts.globals.class_initializers.len()
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut out,
        "  native: extern_funs={}, native_callables={}, extern_globals={}, extern_libs={}",
        facts.native.extern_functions.len(),
        facts.native.native_callables.len(),
        facts.native.extern_globals.len(),
        facts.native.extern_libraries.len()
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut out,
        "  contract_bridge: function_effects={}, calls={}, resumes={}, performs={}, handles={}, assigns={}, updates={}, top_level_roots={}, extern_globals={}",
        facts.contract_bridge.function_effects,
        facts.contract_bridge.call_site_contracts,
        facts.contract_bridge.continuation_resume_sites,
        facts.contract_bridge.perform_sites,
        facts.contract_bridge.handle_sites,
        facts.contract_bridge.assign_place_contracts,
        facts.contract_bridge.with_update_contracts,
        facts.contract_bridge.top_level_init_roots,
        facts.contract_bridge.extern_global_contracts,
    )
    .expect("writing to String cannot fail");
    write!(
        &mut out,
        "  type_context: universe={}, type_params={}, source_cones={}\n}}",
        facts
            .type_context
            .type_universe
            .as_ref()
            .map(|reference| reference.label.as_str())
            .unwrap_or("<none>"),
        facts.type_context.stable_type_params.len(),
        facts.type_context.source_cones.len()
    )
    .expect("writing to String cannot fail");

    out
}
