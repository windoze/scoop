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
    // Identity comparisons are bounded by the fixed-width persistent key;
    // representation lookup and shape traversal use the actual table sizes.
    meter
        .charge_work(128, &WirePath::root())
        .map_err(Error::Resource)?;
    let node = graph.get(inheritance_exact).ok_or(Error::ConcreteSupport)?;
    let interface = record.payload().source_interface();
    if node.source() != record.declaration() || node.edges().modality() != interface.modality() {
        return Err(Error::ConcreteSupport);
    }
    meter
        .charge_work(
            u64::from(representations.records().len().max(1).ilog2()) + 1,
            &WirePath::root(),
        )
        .map_err(Error::Resource)?;
    let representation = representations
        .get(representation_owner)
        .ok_or(Error::ConcreteSupport)?;
    compare(
        representation.declaration_access(),
        record.declaration_access(),
        meter,
    )?;
    let shape_bytes = scoop_wire::encoded_length(interface.source_shape())
        .map_err(Error::Encoding)?
        .saturating_add(
            scoop_wire::encoded_length(representation.shape()).map_err(Error::Encoding)?,
        );
    meter
        .charge_work(shape_bytes, &WirePath::root())
        .map_err(Error::Resource)?;
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
    charge_sort(supertypes.len(), meter).map_err(Error::Resource)?;
    supertypes.sort_unstable();
    let interfaces = node.edges().direct_interfaces();
    let has_base = matches!(
        node.edges().direct_base(),
        DirectClassBaseV1::ClassBase { .. }
    );
    let mut expected = Vec::new();
    meter
        .try_reserve_collection_slots(
            &mut expected,
            interfaces.len() + usize::from(has_base),
            &WirePath::root(),
        )
        .map_err(Error::Resource)?;
    expected.extend_from_slice(interfaces);
    if let DirectClassBaseV1::ClassBase { exact } = node.edges().direct_base() {
        expected.push(exact);
    }
    charge_sort(expected.len(), meter).map_err(Error::Resource)?;
    expected.sort_unstable();
    meter
        .charge_work(expected.len() as u64, &WirePath::root())
        .map_err(Error::Resource)?;
    if supertypes != expected {
        return Err(Error::ConcreteSupport);
    }
    Ok(())
}

fn charge_sort(count: usize, meter: &mut BudgetMeter) -> Result<(), scoop_wire::WireError> {
    let work = (count as u64).saturating_mul(u64::from(count.max(1).ilog2()) + 1);
    meter.charge_work(work, &WirePath::root())
}
