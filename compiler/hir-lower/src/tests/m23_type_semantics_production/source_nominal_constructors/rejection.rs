use super::*;

#[test]
fn nominal_constructor_projection_rejects_missing_or_inconsistent_parameters() {
    with_source(SOURCE, |output, _| {
        let export = &output.output().export;
        let required = required(output);
        let index = export
            .source_parameter_interfaces
            .iter()
            .position(|interface| {
                matches!(
                    interface.owner,
                    hir::ExportParameterOwner::ClassConstructor(_)
                ) && interface.parameters.len() == 2
            })
            .unwrap();
        for corruption in ["missing", "duplicate", "arity", "type", "name"] {
            let mut module = export.clone().into_module();
            match corruption {
                "missing" => {
                    module.source_parameter_interfaces.remove(index);
                }
                "duplicate" => module
                    .source_parameter_interfaces
                    .push(module.source_parameter_interfaces[index].clone()),
                "arity" => {
                    module.source_parameter_interfaces[index]
                        .parameters
                        .pop()
                        .unwrap();
                }
                "type" => {
                    module.source_parameter_interfaces[index].parameters[0].calling =
                        hir::ExportParameterCalling::Required {
                            value_type: module.unit,
                        }
                }
                "name" => {
                    module.source_parameter_interfaces[index].parameters[1].name =
                        module.source_parameter_interfaces[index].parameters[0]
                            .name
                            .clone()
                }
                _ => unreachable!(),
            }
            let forged =
                hir::ExportHirOutput::try_new(module, export.output_kind().clone()).unwrap();
            assert!(
                matches!(
                    Table::from_export_hir(&forged, &required, &mut meter()),
                    Err(hir::CrossConeTypeSemanticsProductionError::InvalidSourceDeclaration(_))
                ),
                "{corruption}"
            );
        }
    });
}

#[test]
fn nominal_constructor_projection_rejects_missing_required_declarations() {
    let foreign = with_source("public class Foreign", |output, _| {
        *required(output).first().unwrap()
    });
    with_source(SOURCE, |output, _| {
        let mut required = required(output);
        required.insert(foreign);
        assert!(
            matches!(Table::from_export_hir(&output.output().export, &required, &mut meter()),
            Err(hir::CrossConeTypeSemanticsProductionError::InvalidSourceDeclaration(message))
                if message == "required nominal source constructor has no sealed declaration")
        );
    });
}

#[test]
fn nominal_constructor_varargs_keep_the_array_value_type_and_host_binder() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-nominals/parameter-varargs.scoop"
    ));
    let output = lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()]).unwrap();
    let export = output.export.module();
    let (id, _) = export
        .class_constructors
        .iter()
        .find(|(_, c)| export.classes[c.owner].name == "Variadic")
        .unwrap();
    let declaration = export.constructor_identities[id]
        .source_record()
        .unwrap()
        .id();
    let source =
        Table::from_export_hir(&output.export, &BTreeSet::from([declaration]), &mut meter())
            .unwrap();
    let record = source.get(declaration).unwrap();
    let value = record.payload().parameters().parameters()[0].value_type();
    let SignatureTypeKey::NominalApplication { origin, arguments } = value else {
        panic!("Array application")
    };
    let (array, _) = export
        .classes
        .iter()
        .find(|(_, c)| {
            matches!(
                c.representation,
                hir::ClassRepresentation::Intrinsic(hir::IntrinsicTypeDeclaration {
                    kind: hir::IntrinsicTypeKind::Array,
                    ..
                })
            )
        })
        .unwrap();
    assert_eq!(
        *origin,
        export.nominal_identities[array]
            .source()
            .unwrap()
            .generic_id()
            .unwrap()
    );
    assert_eq!(
        arguments.as_slice(),
        &[SignatureTypeKey::Binder { depth: 0, index: 0 }]
    );
}

#[test]
fn nominal_constructor_projection_rejects_a_different_owner_or_self_result() {
    with_source(SOURCE, |output, _| {
        let export = &output.output().export;
        let required = required(output);
        let (owner, _) = export
            .classes
            .iter()
            .find(|(_, c)| c.name == "Envelope")
            .unwrap();
        let (other, _) = export
            .classes
            .iter()
            .find(|(_, c)| c.name == "Static")
            .unwrap();
        let constructor = export.classes[owner].constructors[0];
        for change_owner in [false, true] {
            let mut module = export.clone().into_module();
            if change_owner {
                module.class_constructors[constructor].owner = other;
            } else {
                module.classes[owner].self_application = module.classes[other].self_application;
            }
            let forged =
                hir::ExportHirOutput::try_new(module, export.output_kind().clone()).unwrap();
            assert!(matches!(
                Table::from_export_hir(&forged, &required, &mut meter()),
                Err(hir::CrossConeTypeSemanticsProductionError::InvalidSourceDeclaration(_))
            ));
        }
    });
}
