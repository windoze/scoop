use super::*;
use scoop_wire::{decode_canonical, encode};
use source_dispatch::with_hir_source;

mod rejection;

const STANDALONE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-reference-source-fields/standalone.scoop"
));
const COMBINED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-reference-source-fields/combined.scoop"
));

#[test]
fn reference_source_fields_preserve_private_storage_binders_and_declaration_order() {
    for (case, source) in [("standalone", STANDALONE), ("combined", COMBINED)] {
        with_hir_source(source, |output, _| {
            let export = &output.output().export;
            let public = public_interface(output);
            let table = public.nominal_interfaces();
            let foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
            table
                .validate_declared_field_inventory(&foundation)
                .unwrap();
            let mut declarations = std::collections::BTreeSet::new();
            for (class, declaration) in export.classes.iter() {
                let identity = &export.nominal_identities[class];
                let owner = if let Some(source) = identity.source() {
                    hir::SourceNominalId::from_source_declaration(source.declaration()).unwrap()
                } else {
                    let object = export
                        .objects
                        .iter()
                        .find(|(_, object)| object.backing_class == class)
                        .unwrap()
                        .0;
                    hir::SourceNominalId::Concrete(
                        export.nominal_identities[object]
                            .concrete_type_id()
                            .unwrap(),
                    )
                };
                let Some(record) = table.get(owner) else {
                    continue;
                };
                let fields = record.source_shape().declared_fields();
                assert_eq!(fields.len(), declaration.fields.len());
                assert_eq!(
                    fields.iter().map(|field| field.field()).collect::<Vec<_>>(),
                    declaration
                        .fields
                        .iter()
                        .map(|field| export.field_identities[*field].id())
                        .collect::<Vec<_>>()
                );
                if declaration.name == "FieldBox" {
                    assert_eq!(fields.len(), 2);
                    assert!(fields.iter().all(|field| *field.value_type()
                        == scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 0 }));
                }
                let expected_kind = if identity.source().is_some() {
                    hir::PublicNominalKindV1::Class
                } else {
                    hir::PublicNominalKindV1::Object
                };
                assert_eq!(record.kind(), expected_kind);
                assert_eq!(
                    record.type_parameters().len_u32() as usize,
                    declaration.type_params.len()
                );
                declarations.insert(declaration.name.as_str());
            }
            let expected = if case == "standalone" {
                vec![
                    "FieldBase",
                    "FieldBox",
                    "FieldChild",
                    "FieldPrivateOwner",
                    "FieldSingleton",
                ]
            } else {
                vec!["FieldCombined", "FieldCombinedSingleton"]
            };
            assert_eq!(declarations.into_iter().collect::<Vec<_>>(), expected);
            let bytes = encode(table).unwrap();
            let decoded: hir::DecodedCanonicalNominalInterfacesV1 =
                decode_canonical(&bytes).unwrap();
            let mut identities = source_inventory::identity_closure(output);
            let restored = decoded.resolve(&mut identities).unwrap();
            assert_eq!(&restored, table);
            restored
                .validate_declared_field_inventory(&foundation)
                .unwrap();
        });
    }
}

#[test]
fn reference_source_field_bytes_ignore_unrelated_arena_allocation() {
    let project = |source: &str| {
        with_hir_source(source, |output, _| {
            encode(public_interface(output).nominal_interfaces()).unwrap()
        })
    };
    assert_eq!(
        project(STANDALONE),
        project(&format!("private class Unrelated {{}}\n{STANDALONE}"))
    );
}
