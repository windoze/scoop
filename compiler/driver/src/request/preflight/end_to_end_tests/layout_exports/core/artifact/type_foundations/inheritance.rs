use super::*;
use hir::{
    CanonicalNominalInheritanceInterfacesV1, NominalInheritanceEdgesV1,
    NominalInheritanceInterfaceV1,
};

pub(super) fn check(checked: CheckedSharedTypeFoundationV1<'_>) {
    checked
        .with_inheritance_graph(&[], |graph| {
            for record in checked.section().inheritance().records() {
                assert_eq!(graph.get(record.owner()).unwrap().edges(), record.edges());
            }
        })
        .unwrap();

    let mut missing = checked.section().inheritance().records().to_vec();
    missing.pop().unwrap();
    assert!(
        matches!(reject(checked, missing), Error::InheritanceInventory(provider) if provider == checked.provider())
    );

    let mut modality = checked.section().inheritance().records().to_vec();
    let first = &modality[0];
    let owner = first.owner();
    let changed = if first.edges().modality() == hir::NominalInheritanceModalityV1::Open {
        hir::NominalInheritanceModalityV1::Final
    } else {
        hir::NominalInheritanceModalityV1::Open
    };
    modality[0] = NominalInheritanceInterfaceV1::try_new(
        NominalInheritanceEdgesV1::try_new(
            owner,
            changed,
            first.edges().direct_base(),
            first.edges().direct_interfaces().to_vec(),
        )
        .unwrap(),
        first.slots().clone(),
        first.protected_members().clone(),
        first.slot_schemas().clone(),
    )
    .unwrap();
    assert!(
        matches!(reject(checked, modality), Error::InheritanceEdges(actual) if actual == owner)
    );
}

fn reject(
    checked: CheckedSharedTypeFoundationV1<'_>,
    records: Vec<NominalInheritanceInterfaceV1>,
) -> Error {
    let source = checked.section();
    let candidate = CrossConeTypeSemanticsSectionV1::new(
        source.exact_facts().clone(),
        source.representation_support().clone(),
        CanonicalNominalInheritanceInterfacesV1::try_new(records).unwrap(),
        source.selected().clone(),
    );
    let foundation = candidate
        .validate_shared_foundation(checked.metadata(), &[])
        .unwrap();
    foundation
        .with_inheritance_graph(&[], |_| ())
        .expect_err("invalid inheritance must fail against shared source declarations")
}
