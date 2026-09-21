use super::*;

#[test]
fn sealed_vararg_sources_preserve_array_values_defaults_and_generic_binders() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-dispatch/parameter-varargs.scoop"
    ));
    let output = lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()]).unwrap();
    let export = output.export.module();
    let (id, class) = export
        .classes
        .iter()
        .find(|(_, c)| c.name == "Variadic")
        .unwrap();
    let owner = export.type_identities
        [export.class_applications[class.self_application].canonical_type]
        .exact()
        .unwrap()
        .id();
    let constructors = class
        .constructors
        .iter()
        .filter_map(|id| {
            export.constructor_identities[*id]
                .source_record()
                .map(|record| record.id())
        })
        .collect();
    let members = class
        .methods
        .iter()
        .map(|function| {
            hir::ProtectedDeclarationRefV1::Callable(
                hir::ProtectedCallableDeclarationRefV1::try_new(
                    declaration(export, hir::ExportParameterOwner::Function(*function)).unwrap(),
                )
                .unwrap(),
            )
        })
        .collect();
    let inventory = hir::CanonicalSourceInheritanceInventoriesV1::try_new(
        vec![
            hir::SourceInheritanceInventoryV1::try_new(
                owner,
                hir::CanonicalPersistentIdsV1::try_new(constructors).unwrap(),
                hir::CanonicalProtectedDeclarationRefsV1::try_new(members).unwrap(),
                hir::CanonicalInheritanceSlotSchemasV1::try_new(vec![]).unwrap(),
                &mut meter(),
            )
            .unwrap(),
        ],
        &mut meter(),
    )
    .unwrap();
    assert!(
        export.nominal_identities[id]
            .source()
            .unwrap()
            .concrete_id()
            .is_some()
    );
    let table = Table::from_export_hir(&output.export, &inventory, &mut meter()).unwrap();
    assert_eq!(
        render_and_verify(export, &table),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-dispatch/parameter-varargs.snap"
        ))
    );
    let (_, array) = export
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
    let hir::Type::Class(array_application) =
        export.types[export.class_applications[array.self_application].canonical_type]
    else {
        panic!("Array template")
    };
    let array_id = export.nominal_identities[export.class_applications[array_application].template]
        .source()
        .unwrap()
        .generic_id()
        .unwrap();
    let mut generic = 0;
    let mut varargs = 0;
    for record in table.records() {
        for parameter in record.parameters() {
            if matches!(
                parameter.calling_kind(),
                hir::ProtectedParameterCallingKindV1::VarargEmpty
                    | hir::ProtectedParameterCallingKindV1::VarargDefault
            ) {
                let scoop_identity::SignatureTypeKey::NominalApplication { origin, arguments } =
                    parameter.shape().value_type()
                else {
                    panic!("vararg logical Array value")
                };
                assert_eq!(*origin, array_id);
                assert_eq!(arguments.as_slice().len(), 1);
                generic += usize::from(matches!(
                    arguments.as_slice()[0],
                    scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 0 }
                ));
                varargs += 1;
            }
        }
    }
    assert_eq!((varargs, generic), (4, 1));
}

#[test]
fn source_parameter_projection_rejects_missing_duplicate_and_mismatched_interfaces() {
    with_source(SOURCE, |output, _| {
        let inventory =
            hir::CanonicalSourceInheritanceInventoriesV1::from_dependency_hir(output, &mut meter())
                .unwrap();
        let expected = table(output);
        let export = &output.output().export;
        let index = export
            .source_parameter_interfaces
            .iter()
            .position(|interface| {
                declaration(export, interface.owner)
                    .and_then(|id| expected.get(id))
                    .is_some_and(|record| !record.parameters().is_empty())
            })
            .unwrap();
        for corruption in ["missing", "duplicate", "arity", "type", "default"] {
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
                "default" => module.export_default_sources = Default::default(),
                _ => unreachable!(),
            }
            let forged =
                hir::ExportHirOutput::try_new(module, export.output_kind().clone()).unwrap();
            assert!(matches!(
                Table::from_export_hir(&forged, &inventory, &mut meter()),
                Err(hir::CrossConeTypeSemanticsProductionError::InvalidSourceDeclaration(_))
            ));
        }
    });
}
