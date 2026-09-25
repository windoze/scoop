use super::*;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/local-owner-arguments.scoop"
));
const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/local-owner-argument-combinations.scoop"
));

#[test]
fn local_default_descriptors_preserve_unused_owner_arguments_through_each_expansion() {
    for combinations in [false, true] {
        let source = if combinations {
            [SOURCE, COMBINATIONS].join("\n")
        } else {
            SOURCE.to_owned()
        };
        with_sources(&source, |output, fixture, sources, core| {
            let module = output.output().export.module();
            let mut expected = vec![
                parameters(module, "ownerLocal"),
                parameters(module, "OwnerLocalHost.forward"),
            ];
            if combinations {
                let mut bridge = parameters(module, "ownerBridge");
                bridge.reverse();
                expected.push(bridge);
                let mut swapped = parameters(module, "OwnerCombinationHost.swapped");
                swapped.reverse();
                expected.push(swapped);
                let string = type_id(module, &hir::Type::String);
                let integer = type_id(module, &hir::Type::Integer(hir::IntegerKind::SIGNED_32));
                let repeated = parameters(module, "OwnerCombinationHost.repeated")[0];
                expected.push(vec![string, repeated]);
                expected.push(vec![integer, repeated]);
                expected.push(vec![
                    string,
                    parameters(module, "OwnerCombinationHost.layered")[0],
                ]);
            }
            let mut actual = Vec::new();
            for (_, descriptor) in module.local_functions.iter() {
                assert_eq!(descriptor.owner_type_arguments.len(), 2);
                let value = descriptor.owner_type_arguments[1];
                let signature = &module.function_types[descriptor.function_type];
                assert!(signature.parameter_types.is_empty());
                assert_eq!(signature.return_type, value);
                assert_eq!(descriptor.captures.len(), 1);
                assert_eq!(descriptor.captures[0].ty, value);
                actual.push(descriptor.owner_type_arguments.clone());
            }
            actual.sort();
            expected.sort();
            assert_eq!(actual, expected);

            let table = templates(output);
            let foundation = fixture.bind().unwrap();
            sources.with_bound(&foundation, core, |members, constructors| {
                let parameters = members
                    .bind_parameter_protocols(constructors, &sources.protocols)
                    .unwrap();
                parameters.bind_default_declarations(&table, &[]).unwrap();
            });
        });
    }
}

fn parameters(module: &hir::Module, name: &str) -> Vec<hir::TypeId> {
    module
        .functions
        .iter()
        .find(|(_, function)| function.name == name)
        .unwrap()
        .1
        .type_params()
        .into_iter()
        .map(|parameter| type_id(module, &hir::Type::Param(parameter.id)))
        .collect()
}

fn type_id(module: &hir::Module, value: &hir::Type) -> hir::TypeId {
    module.types.iter().find(|(_, ty)| *ty == value).unwrap().0
}
