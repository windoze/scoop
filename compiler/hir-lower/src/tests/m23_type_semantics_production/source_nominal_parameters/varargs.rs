use super::*;

#[test]
fn complete_parameter_protocols_preserve_generic_vararg_array_shapes() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-nominals/parameter-varargs.scoop"
    ));
    let output = lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()]).unwrap();
    let export = output.export.module();
    let required = export
        .source_parameter_interfaces
        .iter()
        .filter(|interface| match interface.owner {
            hir::ExportParameterOwner::Function(id) => {
                export.functions[id].name.starts_with("Variadic.")
            }
            hir::ExportParameterOwner::ClassConstructor(id) => {
                export.classes[export.class_constructors[id].owner].name == "Variadic"
            }
            hir::ExportParameterOwner::StructConstructor(_) => false,
            hir::ExportParameterOwner::VariantConstructor(reference) => {
                export.enums[reference.enumeration()].name == "Batches"
            }
        })
        .map(|interface| declaration(export, interface.owner).unwrap())
        .collect();
    let table = Table::from_export_hir(&output.export, &required, &mut meter()).unwrap();
    assert_eq!(
        contracts::verify(&output.export, &table),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/parameter-varargs.snap"
        ))
    );
    let array = export
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
        .unwrap()
        .0;
    let array = export.nominal_identities[array]
        .source()
        .unwrap()
        .generic_id()
        .unwrap();
    let mut varargs = 0;
    let mut generic = 0;
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
                    panic!("Array value");
                };
                assert_eq!(*origin, array);
                assert_eq!(arguments.as_slice().len(), 1);
                generic += usize::from(matches!(
                    arguments.as_slice()[0],
                    scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 0 }
                ));
                varargs += 1;
            }
        }
    }
    assert_eq!((varargs, generic), (6, 4));
}
