use super::*;

#[test]
fn nominal_source_contracts_reject_foreign_required_owner() {
    with_source(DECLARATIONS, |output, _| {
        let hir::CoreProtocols::Imported(core) = &output.output().export.core_protocols else {
            panic!("imported core")
        };
        let foreign = hir::SourceNominalId::Concrete(core.fundamental_types().unit().persistent());
        let required = hir::CanonicalSourceNominalIdsV1::try_new(vec![foreign]).unwrap();
        assert!(matches!(
            Table::from_export_hir(&output.output().export, &required),
            Err(hir::CrossConeTypeSemanticsProductionError::InvalidSourceDeclaration(_))
        ));
    });
}
