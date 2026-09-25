use super::*;

pub(super) fn intersect(
    left: &DefaultSourceAccessDomainV1,
    right: &DefaultSourceAccessDomainV1,

    path: &WirePath,
) -> Result<DefaultSourceAccessDomainV1, Error> {
    if left.is_empty() || right.is_empty() {
        return Ok(DefaultSourceAccessDomainV1::empty());
    }

    let persistent = left
        .persistent()
        .intersect(right.persistent())
        .map_err(Error::PersistentDomain)?;
    let count =
        left.generic_subclasses().values().len() + right.generic_subclasses().values().len();

    let mut generic = Vec::new();
    scoop_wire::allocation::try_reserve(&mut generic, count, path)?;
    generic.extend_from_slice(left.generic_subclasses().values());
    generic.extend_from_slice(right.generic_subclasses().values());
    generic.sort_unstable();
    generic.dedup();
    let generic = CanonicalPersistentIdsV1::try_new(generic).map_err(Error::GenericDomain)?;
    DefaultSourceAccessDomainV1::try_new(persistent, generic).map_err(Error::DomainBuild)
}
