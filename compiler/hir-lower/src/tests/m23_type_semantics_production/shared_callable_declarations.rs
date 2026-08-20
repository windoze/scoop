use super::*;
use hir::{CallableDeclarationInventoryError as Error, CanonicalCallableInterfacesV1 as Table};
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{decode_canonical, encode};
use source_dispatch::with_hir_source;

const STANDALONE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-shared-callable-declarations/standalone.scoop"
));
const COMBINED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-shared-callable-declarations/combined.scoop"
));

#[test]
fn shared_callables_preserve_restricted_signatures_in_ordinary_metadata() {
    for source in [STANDALONE, COMBINED] {
        with_hir_source(source, |output, _| {
            let export = output.output().export.module();
            let nominals = hir::CanonicalNominalInterfacesV1::from_export_hir(export).unwrap();
            let properties = hir::CanonicalPropertyInterfacesV1::from_export_hir(export).unwrap();
            let table = Table::from_export_hir(export).unwrap();
            table
                .validate_declaration_inventory(&nominals, &properties)
                .unwrap();
            assert!(!table.support_records().is_empty());
            for record in table.support_records() {
                assert!(table.get(record.declaration()).is_none());
                assert_eq!(table.declaration(record.declaration()), Some(record));
            }
            let foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
            let mut identities =
                source_inventory::identity_closure_for_foundation(output, foundation);
            let restored: hir::DecodedCanonicalCallableInterfacesV1 =
                decode_canonical(&encode(&table).unwrap()).unwrap();
            assert_eq!(restored.resolve(&mut identities).unwrap(), table);
        });
    }
}

#[test]
fn shared_callables_reject_missing_support_and_cross_partition_duplicates() {
    with_hir_source(COMBINED, |output, _| {
        let export = output.output().export.module();
        let nominals = hir::CanonicalNominalInterfacesV1::from_export_hir(export).unwrap();
        let properties = hir::CanonicalPropertyInterfacesV1::from_export_hir(export).unwrap();
        let table = Table::from_export_hir(export).unwrap();
        for removed in table.support_records() {
            let support = table
                .support_records()
                .iter()
                .filter(|record| record.declaration() != removed.declaration())
                .cloned()
                .collect();
            let missing = Table::with_support(table.records().to_vec(), support).unwrap();
            assert_eq!(
                missing.validate_declaration_inventory(&nominals, &properties),
                Err(Error::Missing(removed.declaration()))
            );
        }
        let public = &table.records()[0];
        let mut duplicate = table.support_records().to_vec();
        duplicate.push(public.declaration_data().clone());
        assert_eq!(
            Table::with_support(table.records().to_vec(), duplicate),
            Err(hir::CallableInterfaceSetBuildError::DuplicateDeclaration(
                public.declaration()
            ))
        );
        let private = table
            .support_records()
            .iter()
            .find(|r| r.declared_visibility() == hir::DeclaredVisibilityV1::Private)
            .unwrap();
        assert_eq!(
            hir::CallableInterfaceRecordV1::from_declaration(
                private.clone(),
                hir::PublicLookupAccessV1::DirectOnly
            ),
            Err(
                hir::CallableInterfaceRecordBuildError::NonPublicDeclaration(private.declaration())
            )
        );
    });
}

#[test]
fn shared_callables_reject_wrong_owner_and_unrelated_support() {
    with_hir_source(STANDALONE, |output, _| {
        let export = output.output().export.module();
        let nominals = hir::CanonicalNominalInterfacesV1::from_export_hir(export).unwrap();
        let properties = hir::CanonicalPropertyInterfacesV1::from_export_hir(export).unwrap();
        let table = Table::from_export_hir(export).unwrap();
        let mut support = table.support_records().to_vec();
        let record = support
            .iter_mut()
            .find(|r| matches!(r.declaration(), CallableTemplateOrigin::Function(_)))
            .unwrap();
        let declaration = record.declaration();
        let hir::PublicDeclarationOwnerV1::Nominal(expected) = record.owner() else {
            panic!("nominal")
        };
        *record = hir::CallableDeclarationRecordV1::try_new(
            declaration,
            hir::PublicDeclarationOwnerV1::TopLevel,
            record.type_parameters().clone(),
            record.receiver().cloned(),
            record.parameters().clone(),
            record.result().clone(),
            record.effects(),
            record.modality(),
            record.declared_visibility(),
            record.slot_relations().clone(),
            Vec::new(),
        )
        .unwrap();
        let invalid = Table::with_support(table.records().to_vec(), support).unwrap();
        assert_eq!(
            invalid.validate_declaration_inventory(&nominals, &properties),
            Err(Error::Owner {
                declaration,
                expected: hir::PublicDeclarationOwnerV1::Nominal(expected),
                actual: hir::PublicDeclarationOwnerV1::TopLevel
            })
        );
        assert!(matches!(
            table.validate_declaration_inventory(
                &hir::CanonicalNominalInterfacesV1::try_new(vec![]).unwrap(),
                &properties
            ),
            Err(Error::UnexpectedSupport(_))
        ));
    });
}
