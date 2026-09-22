use super::*;
use hir::{DefaultCoreApplicationV1 as Application, DefaultOperationCoreTypeV1 as Role};
use scoop_identity::{NonEmptyVec, SignatureTypeKey as Type};
use scoop_wire::WirePath;
mod resources;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-default-operation-types/standalone.scoop"
));
const COMBINED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-default-operation-types/combined.scoop"
));

fn with_protocols(source: &str, run: impl FnOnce(&hir::Output, &hir::ImportedCoreProtocols)) {
    use super::core_foundation::support::{artifact, import, lower_extra};
    let output = lower_extra(source);
    let definitions = hir::CompilerProtocolDefinitionsV1::from_export(&output.export).unwrap();
    let (foundation, identities) = artifact(&output);
    let imported = import(&foundation, &identities);
    let inputs = imported.import_core_inputs(&definitions).unwrap();
    run(&output, inputs.protocols());
}
fn defaults(output: &hir::Output) -> Vec<(String, Type)> {
    let export = &output.export;
    export
        .source_parameter_interfaces
        .iter()
        .filter_map(|p| {
            let hir::ExportParameterOwner::Function(id) = p.owner else {
                return None;
            };
            let name = &export.functions[id].name;
            if !name.starts_with("Values.") && !name.starts_with("Combinations.") {
                return None;
            }
            let source = hir::DefaultSourceBodyProductionV1::from_export_hir(
                export,
                p.owner,
                1,
                &mut meter(),
            )
            .unwrap();
            Some((
                export.functions[id]
                    .name
                    .rsplit('.')
                    .next()
                    .unwrap()
                    .to_owned(),
                source.result().clone(),
            ))
        })
        .collect()
}

#[test]
fn operation_types_replay_all_real_imported_language_roles() {
    use hir::DefaultIntegerKindV1 as Integer;
    let roles = [
        ("unit", Role::Unit),
        ("boolean", Role::Boolean),
        ("string", Role::String),
        ("byte", Role::Integer(Integer::Signed8)),
        ("short", Role::Integer(Integer::Signed16)),
        ("int", Role::Integer(Integer::Signed32)),
        ("long", Role::Integer(Integer::Signed64)),
        ("ubyte", Role::Integer(Integer::Unsigned8)),
        ("ushort", Role::Integer(Integer::Unsigned16)),
        ("uint", Role::Integer(Integer::Unsigned32)),
        ("ulong", Role::Integer(Integer::Unsigned64)),
        ("throwable", Role::Throwable),
        ("state", Role::ForeignCallbackState),
    ]
    .into_iter()
    .collect::<std::collections::BTreeMap<_, _>>();
    with_protocols(SOURCE, |output, protocols| {
        let defaults = defaults(output);
        assert_eq!(defaults.len(), roles.len());
        let mut dump = Vec::new();
        for (name, actual) in defaults {
            let role = roles[name.as_str()];
            assert_eq!(
                protocols
                    .default_operation_type(role, &mut meter(), &WirePath::root())
                    .unwrap(),
                actual
            );
            assert!(
                protocols
                    .classify_default_operation_application(
                        &actual,
                        &mut meter(),
                        &WirePath::root()
                    )
                    .unwrap()
                    .is_none()
            );
            dump.push(format!("{name}: {role:?}"));
        }
        dump.sort();
        assert_eq!(
            dump.join("\n") + "\n",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-default-operation-types/standalone.snap"
            ))
        );
    });
}

fn classified(value: &Application) -> (&'static str, &Type) {
    match value {
        Application::Array { element } => ("Array", element),
        Application::MutableArray { element } => ("MutableArray", element),
        Application::Option { element } => ("Option", element),
        Application::ForeignCallback { function_type } => ("ForeignCallback", function_type),
    }
}

#[test]
fn operation_applications_keep_open_arguments_and_reject_same_named_ordinary_types() {
    with_protocols(COMBINED, |output, protocols| {
        let mut dump = Vec::new();
        for (name, actual) in defaults(output) {
            let result = protocols
                .classify_default_operation_application(&actual, &mut meter(), &WirePath::root())
                .unwrap();
            if name.starts_with("mimic") || name == "tuple" {
                assert!(result.is_none());
                dump.push(format!("{name}: ordinary"));
                continue;
            }
            let result = result.unwrap();
            let (kind, argument) = classified(&result);
            let Type::NominalApplication { origin, arguments } = actual else {
                panic!("protocol application");
            };
            assert_eq!(argument, &arguments.as_slice()[0]);
            let wrong_arity = Type::NominalApplication {
                origin,
                arguments: NonEmptyVec::from_first(argument.clone(), [argument.clone()]),
            };
            assert!(
                matches!(protocols.classify_default_operation_application(&wrong_arity, &mut meter(), &WirePath::root()), Err(hir::DefaultOperationProtocolTypeError::Arity { origin: actual, actual: 2 }) if actual == origin)
            );
            dump.push(format!("{name}: {kind}"));
        }
        dump.sort();
        assert_eq!(
            dump.join("\n") + "\n",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-default-operation-types/combined.snap"
            ))
        );
    });
}
