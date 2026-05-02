use std::fmt::Write as _;

use super::MaterializedEffectFacts;

pub fn render_materialized_effect_facts(facts: &MaterializedEffectFacts) -> String {
    let binding = facts.snapshot_binding();
    let mut rendered = String::new();
    let _ = writeln!(&mut rendered, "MaterializedEffectFacts {{");
    let _ = writeln!(
        &mut rendered,
        "  query_surface: {:?},",
        binding.query_surface()
    );
    let _ = writeln!(
        &mut rendered,
        "  instance_count: {},",
        binding.instance_count()
    );
    let _ = writeln!(
        &mut rendered,
        "  canonical_body_fqns: {:?},",
        binding.canonical_body_fqns()
    );
    let _ = writeln!(
        &mut rendered,
        "  step_schemas: {},",
        facts.step_schemas().len()
    );
    let _ = writeln!(
        &mut rendered,
        "  continuation_schemas: {},",
        facts.continuation_schemas().len()
    );
    let _ = writeln!(
        &mut rendered,
        "  callable_facts: {},",
        facts.callable_facts().len()
    );
    let _ = writeln!(&mut rendered, "  bodies: {},", facts.bodies().len());
    let _ = writeln!(&mut rendered, "}}");
    rendered
}
