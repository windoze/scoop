use super::*;

#[test]
fn restricted_setters_keep_complete_source_metadata_for_every_owner_kind() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-setter-domains/combined.scoop"),
    )
    .unwrap();
    source_dispatch::with_hir_source(&source, |output, _| {
        let public = public_interface(output);
        assert_eq!(public.property_interfaces().declaration_count(), 10);
        assert_eq!(public.property_interfaces().support_records().len(), 8);
        let unit = scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id();
        let unit = public
            .external_references()
            .get(hir::ExternalHirTargetV1::Nominal(
                hir::SourceNominalId::Concrete(unit),
            ))
            .expect("restricted setter result types contribute external signature references");
        assert!(
            unit.roles()
                .contains(hir::ExternalHirReferenceRoleV1::SignatureDependency)
        );
    });
}
