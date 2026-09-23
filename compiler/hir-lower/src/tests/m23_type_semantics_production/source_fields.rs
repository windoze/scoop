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

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn reference_source_fields_preserve_private_storage_binders_and_declaration_order() {
    for (case, source) in [("standalone", STANDALONE), ("combined", COMBINED)] {
        with_hir_source(source, |output, _| {
            let export = &output.output().export;
            let public = public_interface(output);
            let table = public.nominal_interfaces();
            let foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
            table
                .validate_declared_field_inventory(&foundation, &mut meter())
                .unwrap();
            let required =
                hir::CanonicalSourceNominalIdsV1::from_export_hir(export, &mut meter()).unwrap();
            let contracts = hir::CanonicalNominalSourceContractsV1::from_export_hir(
                export,
                &required,
                &mut meter(),
            )
            .unwrap();
            let mut dump = Vec::new();
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
                assert_eq!(
                    record.source_shape(),
                    contracts.get(owner).unwrap().source_shape()
                );
                if declaration.name == "FieldBox" {
                    assert_eq!(fields.len(), 2);
                    assert!(fields.iter().all(|field| *field.value_type()
                        == scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 0 }));
                }
                dump.push(format!(
                    "{} {:?} binders={}\n{}",
                    declaration.name,
                    record.kind(),
                    record.type_parameters().len_u32(),
                    fields
                        .iter()
                        .map(|field| format!("  {} {:?}\n", field.field(), field.value_type()))
                        .collect::<String>()
                ));
            }
            dump.sort();
            assert_snapshot(case, &dump.concat());
            let bytes = encode(table).unwrap();
            let decoded: hir::DecodedCanonicalNominalInterfacesV1 =
                decode_canonical(&bytes, DecodeLimits::default()).unwrap();
            let mut identities = source_inventory::identity_closure(output);
            let restored = decoded.resolve(&mut identities).unwrap();
            assert_eq!(&restored, table);
            restored
                .validate_declared_field_inventory(&foundation, &mut meter())
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

fn assert_snapshot(case: &str, actual: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../tests/fixtures/m23-reference-source-fields/{case}.snap"
    ));
    if std::env::var_os("SCOOP_UPDATE_SOURCE_FIELDS_SNAPSHOTS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    assert_eq!(actual, std::fs::read_to_string(path).unwrap());
}
