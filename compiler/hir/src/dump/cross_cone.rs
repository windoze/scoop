use std::fmt::{Debug, Write};

use crate::{CrossConeHirInterfaceSectionV1, DependencyHirOutput};

/// Shows the current HIR publication and source selections in their existing
/// canonical or arena order. No dependency graph is reconstructed for a dump.
pub fn dump_cross_cone(
    output: &DependencyHirOutput,
    interface: &CrossConeHirInterfaceSectionV1,
) -> String {
    let export = output.output().export.module();
    let mut text = format!("CrossCone {}\n", export.cone);
    macro_rules! arena {
        ($($field:ident),* $(,)?) => {$(
            for (id, value) in export.$field.iter() {
                writeln!(text, "  {} {id:?} {value:?}", stringify!($field))
                    .expect("writing to String");
            }
        )*};
    }
    arena!(
        class_constructors,
        struct_constructors,
        imported_generic_applications,
    );
    macro_rules! table {
        ($($field:ident),* $(,)?) => {$(
            records(&mut text, stringify!($field), interface.$field().records());
        )*};
    }
    table!(
        public_bindings,
        nominal_interfaces,
        callable_interfaces,
        property_interfaces,
        type_aliases,
        source_interfaces,
        default_templates,
        constants,
        external_references,
        generic_callable_bodies,
        generic_initializations,
        generic_delegates,
    );
    records(
        &mut text,
        "nominal_support",
        interface.nominal_interfaces().support_records(),
    );
    records(
        &mut text,
        "callable_support",
        interface.callable_interfaces().support_records(),
    );
    records(
        &mut text,
        "property_support",
        interface.property_interfaces().support_records(),
    );
    records(
        &mut text,
        "definition_sources",
        interface.definition_sources().sources(),
    );
    if !interface.annotations().declarations().is_empty()
        || !interface.annotations().targets().is_empty()
    {
        records(
            &mut text,
            "annotations",
            interface.annotations().declarations(),
        );
        records(
            &mut text,
            "annotated_targets",
            interface.annotations().targets(),
        );
    }
    text
}

fn records(text: &mut String, name: &str, records: &[impl Debug]) {
    writeln!(text, "  {name} count={}", records.len()).expect("writing to String");
    for (index, record) in records.iter().enumerate() {
        writeln!(text, "    {name}[{index}] {record:?}").expect("writing to String");
    }
}
