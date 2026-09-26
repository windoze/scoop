use super::*;
use scoop_identity::SignatureTypeKey;

#[test]
fn modified_core_preserves_generic_vararg_source_signatures() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    for (directory, expected) in [
        ("m23-type-source-nominals", 6),
        ("m23-type-source-dispatch", 4),
    ] {
        let source =
            std::fs::read_to_string(fixtures.join(directory).join("parameter-varargs.scoop"))
                .unwrap();
        let module = lower_core_with_additional_declarations(
            scoop_parser::parse(&source).unwrap().declarations,
        );
        let public = hir::CanonicalNominalInterfacesV1::from_export_hir(&module).unwrap();
        let source = hir::CanonicalCallableSourceInterfacesV1::from_export_hir(&module).unwrap();
        let mut varargs = 0;
        for record in source.records() {
            for parameter in record.parameters().parameters() {
                let Some(element) = parameter.calling().element_type() else {
                    continue;
                };
                let SignatureTypeKey::NominalApplication { origin, arguments } =
                    parameter.value_type()
                else {
                    panic!("a vararg parameter must retain its actual Array application");
                };
                assert_eq!(arguments.as_slice(), std::slice::from_ref(element));
                assert_eq!(
                    public
                        .get(NominalDeclarationOwner::GenericTemplate(*origin))
                        .unwrap()
                        .source_shape(),
                    &hir::NominalSourceShapeV1::Intrinsic(
                        hir::NominalIntrinsicRepresentationV1::new(hir::IntrinsicTypeKind::Array)
                    ),
                );
                varargs += 1;
            }
        }
        assert_eq!(varargs, expected, "{directory}");
    }
}

#[test]
fn modified_core_closes_object_enum_and_compound_storage_declarations() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-nominals/storage-roots.scoop"
    ));
    // Ptr, FunPtr, and Option are local declarations in this source test.
    let module =
        lower_core_with_additional_declarations(scoop_parser::parse(source).unwrap().declarations);
    let public = hir::CanonicalNominalInterfacesV1::from_export_hir(&module).unwrap();
    let expected = [
        "Box",
        "Fields",
        "Payload",
        "Result",
        "Selection",
        "Storage",
        "Target",
    ];
    let names = module
        .structs
        .iter()
        .map(|(id, value)| (&value.name, &module.nominal_identities[id]))
        .chain(
            module
                .classes
                .iter()
                .map(|(id, value)| (&value.name, &module.nominal_identities[id])),
        )
        .chain(
            module
                .enums
                .iter()
                .map(|(id, value)| (&value.name, &module.nominal_identities[id])),
        )
        .chain(
            module
                .objects
                .iter()
                .map(|(id, value)| (&value.name, &module.nominal_identities[id])),
        );
    let mut actual = Vec::new();
    for (name, identity) in names {
        if name != "Unrelated" && !expected.contains(&name.as_str()) {
            continue;
        }
        // Generated object backing classes are not source declarations.
        let Some(source) = identity.source() else {
            continue;
        };
        let owner = hir::SourceNominalId::from_source_declaration(source.declaration()).unwrap();
        if public.declaration(owner).is_some() {
            actual.push(name.as_str());
        }
    }
    actual.sort();
    assert_eq!(actual, expected);
}
