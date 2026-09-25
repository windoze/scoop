use super::*;
use crate::cross_cone_type_semantics::inheritance::is_nominal_ancestor;

/// Receiver ancestry includes interface edges. Object backing classes never
/// substitute for source object IDs in this source-level lookup relation.
pub(super) fn validate<E>(
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    receiver: PersistentExactTypeId,
    owner: PersistentExactTypeId,

    path: &WirePath,
) -> Result<(), Error<E>> {
    let reaches = is_nominal_ancestor(receiver, owner, path, |current| {
        let edges = graph.get(current).ok_or(Error::<E>::Receiver)?.edges();
        let base = match edges.direct_base() {
            DirectClassBaseV1::NoClassBase => None,
            DirectClassBaseV1::ClassBase { exact } => Some(exact),
        };
        Ok::<_, Error<E>>(edges.direct_interfaces().iter().copied().chain(base))
    })?;
    if reaches {
        Ok(())
    } else {
        Err(Error::Receiver)
    }
}
