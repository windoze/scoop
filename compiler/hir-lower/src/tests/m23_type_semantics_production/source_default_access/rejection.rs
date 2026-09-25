use super::*;
use hir::DefaultSourceAccessBuildError as Build;
use scoop_identity::CallableTemplateOrigin;

fn witness_bytes(
    owner: CallableTemplateOrigin,
    slot: &hir::OptionalDefaultSourceSlotDomainV1,
) -> Vec<u8> {
    [
        vec![0xa4, 1],
        encode(&owner).unwrap(),
        vec![2],
        encode(&Domain::universal()).unwrap(),
        vec![3],
        encode(slot).unwrap(),
        vec![4],
        encode(&Domain::universal()).unwrap(),
    ]
    .concat()
}
#[test]
fn witness_reader_rejects_accessor_owners_and_constructor_slots() {
    with_hir_source(SOURCE, |output, _| {
        let export = output.output().export.module();
        let getter = export.property_getters.iter().next().unwrap().0;
        let accessor = export
            .property_accessor_identities
            .get_getter(getter)
            .unwrap()
            .id();
        let ctor = export.classes[class(export, "LocalBase")]
            .constructors
            .iter()
            .find_map(|id| {
                export.constructor_identities[*id]
                    .source_record()
                    .map(|record| record.id())
            })
            .unwrap();
        for (owner, slot, expected) in [
            (
                CallableTemplateOrigin::Accessor(accessor),
                hir::OptionalDefaultSourceSlotDomainV1::Absent,
                Build::AccessorOwner,
            ),
            (
                CallableTemplateOrigin::Constructor(ctor),
                hir::OptionalDefaultSourceSlotDomainV1::Present(Domain::empty()),
                Build::SlotForConstructor,
            ),
        ] {
            assert_eq!(
                Witness::try_new(
                    owner,
                    Domain::universal(),
                    slot.clone(),
                    Domain::universal()
                )
                .unwrap_err(),
                expected
            );
            let bytes = witness_bytes(owner, &slot);
            let decoded: DecodedWitness = decode_canonical(&bytes).unwrap();
            assert!(
                matches!(decoded.resolve(&mut identity_closure(output)), Err(hir::DefaultSourceAccessResolutionError::Build(error)) if error == expected)
            );
            for count in [0xa3, 0xa5] {
                let mut corrupt = bytes.clone();
                corrupt[0] = count;
                assert!(decode_canonical::<DecodedWitness>(&corrupt).is_err());
            }
        }
    });
}

#[test]
fn private_literal_fixture_preserves_its_file_constraint_and_source_provider() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/literal.scoop"
    ));
    with_hir_source(source, |output, _| {
        let export = output.output().export.module();
        let body = hir::DefaultSourceBodyProductionV1::from_dependency_hir(
            output,
            function(export, "hidden"),
            0,
        )
        .unwrap();
        for source in witnesses(body.source_references()) {
            let witness = Witness::from_export_hir(export, source).unwrap();
            assert_eq!(witness.owner(), body.owner());
            assert!(witness.target_domain().is_universal());
            assert!(witness.direct_call_domain().persistent().constraints().iter().any(|constraint| matches!(constraint, hir::PersistentAccessConstraintV1::File(file) if file == &export.source_files[0].identity)));
        }
    });
}
