use super::*;
use crate::{DirectClassBaseV1, NestedNominalSupportV1};
use scoop_identity::{ExactTypeKey, PersistentExactTypeId, SignatureTypeKey};

pub(super) fn validate<E>(
    record: &NominalSupportNestedInterfaceV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    representations: &CanonicalNominalRepresentationSupportV1,
    meter: &mut BudgetMeter,
) -> Result<(), NestedSourceSemanticError<E>> {
    use NestedSourceSemanticError as Error;
    let NestedNominalSupportV1::ParamFree {
        inheritance_exact,
        representation_owner,
    } = record.payload().support()
    else {
        return Ok(());
    };
    let node = graph.get(inheritance_exact).ok_or(Error::ConcreteSupport)?;
    let interface = record.payload().source_interface();
    if node.source() != record.declaration() || node.edges().modality() != interface.modality() {
        return Err(Error::ConcreteSupport);
    }
    let representation = representations
        .get(representation_owner)
        .ok_or(Error::ConcreteSupport)?;
    compare(
        representation.declaration_access(),
        record.declaration_access(),
        meter,
    )?;
    representation
        .validate_public_source_shape(interface.source_shape())
        .map_err(|_| Error::ConcreteSupport)?;
    let mut supertypes = Vec::new();
    meter
        .try_reserve_collection_slots(
            &mut supertypes,
            interface.supertypes().values().len(),
            &WirePath::root(),
        )
        .map_err(Error::Resource)?;
    for supertype in interface.supertypes().values() {
        let SignatureTypeKey::Nominal(id) = supertype else {
            return Err(Error::ConcreteSupport);
        };
        meter
            .charge_sha256(40, &WirePath::root())
            .map_err(Error::Resource)?;
        let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(*id))
            .map_err(|_| Error::ConcreteSupport)?;
        supertypes.push(exact);
    }
    supertypes.sort_unstable();
    let mut expected = node.edges().direct_interfaces().to_vec();
    if let DirectClassBaseV1::ClassBase { exact } = node.edges().direct_base() {
        expected.push(exact);
    }
    expected.sort_unstable();
    if supertypes != expected {
        return Err(Error::ConcreteSupport);
    }
    Ok(())
}
