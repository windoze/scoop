use super::*;
use crate::{DirectClassBaseV1, NestedNominalSupportV1};
use scoop_identity::{ExactTypeKey, PersistentExactTypeId, SignatureTypeKey};

pub(super) fn validate<E>(
    record: &NominalSupportNestedInterfaceV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    representations: &CanonicalNominalRepresentationSupportV1,
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
    )?;

    representation
        .validate_public_source_shape(interface.source_shape())
        .map_err(|_| Error::ConcreteSupport)?;
    let mut supertypes = Vec::new();
    scoop_wire::allocation::try_reserve(
        &mut supertypes,
        interface.supertypes().values().len(),
        &WirePath::root(),
    )
    .map_err(Error::Resource)?;
    for supertype in interface.supertypes().values() {
        let SignatureTypeKey::Nominal(id) = supertype else {
            return Err(Error::ConcreteSupport);
        };

        let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(*id))
            .map_err(|_| Error::ConcreteSupport)?;
        supertypes.push(exact);
    }

    supertypes.sort_unstable();
    let interfaces = node.edges().direct_interfaces();
    let has_base = matches!(
        node.edges().direct_base(),
        DirectClassBaseV1::ClassBase { .. }
    );
    let mut expected = Vec::new();
    scoop_wire::allocation::try_reserve(
        &mut expected,
        interfaces.len() + usize::from(has_base),
        &WirePath::root(),
    )
    .map_err(Error::Resource)?;
    expected.extend_from_slice(interfaces);
    if let DirectClassBaseV1::ClassBase { exact } = node.edges().direct_base() {
        expected.push(exact);
    }

    expected.sort_unstable();

    if supertypes != expected {
        return Err(Error::ConcreteSupport);
    }
    Ok(())
}
