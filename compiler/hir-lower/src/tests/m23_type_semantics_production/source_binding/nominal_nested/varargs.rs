use super::*;
use scoop_identity::SignatureTypeKey;

#[test]
fn nested_production_preserves_vararg_arrays_elements_and_default_owners() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-nominals/nested-varargs.scoop"
    ));
    let output = lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()]).unwrap();
    let export = &output.export;
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
    let class = production::class(export, "Variadic");
    let enumeration = export
        .enums
        .iter()
        .find(|(_, e)| e.name == "Batches")
        .unwrap()
        .0;
    let enumeration = hir::SourceNominalId::from_source_declaration(
        export.nominal_identities[enumeration]
            .source()
            .unwrap()
            .declaration(),
    )
    .unwrap();
    let mut counts = [0; 5];
    for root in [class, enumeration] {
        let produced =
            hir::NestedNominalSourceProductionV1::from_export_hir(export, root, &mut meter())
                .unwrap();
        let mut expected = BTreeSet::new();
        collect_protocols(produced.record(), &mut expected);
        assert_eq!(
            produced
                .protocols()
                .records()
                .iter()
                .map(|r| r.owner())
                .collect::<BTreeSet<_>>(),
            expected
        );
        for protocol in produced.protocols().records() {
            for (position, parameter) in protocol.parameters().parameters().iter().enumerate() {
                if let Some(key) = parameter.calling().template() {
                    assert_eq!(
                        key,
                        hir::ProtectedDefaultTemplateKeyV1::try_new(
                            protocol.owner(),
                            position as u32
                        )
                        .unwrap()
                    );
                    counts[4] += 1;
                }
                let element = match parameter.calling() {
                    hir::ProtectedParameterCallingV1::VarargEmpty { element_type }
                    | hir::ProtectedParameterCallingV1::VarargDefault { element_type, .. } => {
                        element_type
                    }
                    _ => continue,
                };
                let SignatureTypeKey::NominalApplication { origin, arguments } =
                    parameter.value_type()
                else {
                    panic!("vararg Array value");
                };
                assert_eq!(*origin, array);
                assert_eq!(arguments.as_slice(), std::slice::from_ref(element));
                counts[0] += 1;
                counts[1] += usize::from(matches!(
                    element,
                    SignatureTypeKey::Binder { depth: 0, index: 0 }
                ));
                counts[2] += usize::from(matches!(element, SignatureTypeKey::Function { .. }));
                counts[3] += usize::from(matches!(element, SignatureTypeKey::Tuple(_)));
            }
        }
    }
    assert_eq!(counts, [8, 4, 1, 1, 3]);
}
