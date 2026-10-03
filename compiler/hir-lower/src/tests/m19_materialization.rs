use super::*;
use scoop_identity::CallableTemplateOwner;
use std::collections::BTreeMap;

#[test]
fn constructor_materialization_closes_selected_bodies_and_keeps_source_contracts() {
    for (case, source) in [
        (
            "selection",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-materialization/selection.scoop"
            )),
        ),
        (
            "closure",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m19-constructor-materialization/closure.scoop"
            )),
        ),
    ] {
        let output = lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()]).unwrap();
        let mut declarations = BTreeMap::new();
        let mut declared_counts = [0; 2];
        for (id, constructor) in output.export.class_constructors.iter() {
            if output.export.classes[constructor.owner]
                .name
                .starts_with("Deferred")
            {
                let identity = output.export.constructor_identities[id]
                    .source_record()
                    .unwrap();
                declarations.insert(identity.id(), constructor.parameters.len());
                declared_counts[0] += 1;
            }
        }
        for (id, constructor) in output.export.struct_constructors.iter() {
            if output.export.structs[constructor.owner]
                .name
                .starts_with("Deferred")
            {
                declarations.insert(
                    output.export.constructor_identities[id].id(),
                    constructor.parameters.len(),
                );
                declared_counts[1] += 1;
            }
        }
        assert_eq!(
            declared_counts,
            if case == "selection" { [3, 2] } else { [11, 2] }
        );
        if case == "selection" {
            assert!(
                output
                    .local
                    .classes
                    .iter()
                    .any(|(_, c)| c.name == "DeferredTypeOnly")
            );
        }
        // The unused overload must not instantiate its GC-free helper for String.
        assert!(
            output
                .local
                .functions
                .iter()
                .all(|(_, f)| !f.name.starts_with("gcFree"))
        );
        let mut selected = Vec::new();
        let mut concrete_counts = [0; 2];
        for (_, constructor) in output.local.class_constructors.iter() {
            if output.local.classes[constructor.class]
                .name
                .starts_with("Deferred")
            {
                selected.push((
                    constructor.materialization,
                    constructor.parameters.len(),
                    true,
                ));
                concrete_counts[0] += 1;
            }
        }
        for (_, constructor) in output.local.struct_constructors.iter() {
            if output.local.structs[constructor.structure]
                .name
                .starts_with("Deferred")
            {
                selected.push((
                    constructor.materialization,
                    constructor.parameters.len(),
                    false,
                ));
                concrete_counts[1] += 1;
            }
        }
        assert_eq!(
            concrete_counts,
            if case == "selection" { [1, 1] } else { [9, 2] }
        );
        let mir = scoop_mir_lower::lower(&output.local).unwrap();
        for (materialization, parameters, class) in selected {
            let CallableTemplateOwner::Constructor(declaration) = materialization.template() else {
                panic!("selected source constructor")
            };
            assert_eq!(declarations[&declaration], parameters);
            let roots = mir
                .meta
                .source_callable_materializations
                .iter()
                .filter(|root| root.materialization() == materialization)
                .collect::<Vec<_>>();
            assert_eq!(roots.len(), 1);
            let function = &mir.functions[roots[0].function()];
            assert_eq!(function.params.len(), parameters + usize::from(class));
            assert_eq!(
                function.gc_effect,
                if class || parameters == 2 {
                    scoop_mir::GcEffect::Managed
                } else {
                    scoop_mir::GcEffect::NoGc
                }
            );
        }
    }
}
