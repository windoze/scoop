//! Shared source visibility replay. These raw domains do not grant lookup authority.
use crate::*;
use scoop_identity::{
    ExactTypeKey, PersistentExactTypeId, PersistentGenericTypeId, SourceDeclarationKey,
    SourceDeclarationKind,
};
use scoop_wire::{WireError, WirePath};
mod authority;
mod builder;
mod errors;
pub use errors::DefaultSourceDomainReplayError;
type Error<E> = DefaultSourceDomainReplayError<E>;

pub(in crate::cross_cone_type_semantics::source_authority) trait SourceDomainAuthority {
    type Error;
    fn nominal_key(&self, owner: SourceNominalId) -> Result<&SourceDeclarationKey, Self::Error>;
    fn nominal_access(
        &self,
        owner: SourceNominalId,
    ) -> Result<&DeclarationAccessSourceV1, Self::Error>;
    fn exact_key(&self, exact: PersistentExactTypeId) -> Result<&ExactTypeKey, Self::Error>;
}
pub(in crate::cross_cone_type_semantics::source_authority) fn lookup_domain<
    A: SourceDomainAuthority,
>(
    access: &DeclarationAccessSourceV1,
    authority: &A,

    path: &WirePath,
) -> Result<DefaultSourceAccessDomainV1, Error<A::Error>> {
    let mut builder = builder::Builder::new(authority, access.lexical_owners().len() + 1, path)?;
    builder.declared(access)?;
    for owner in access.lexical_owners() {
        let outer = authority.nominal_access(*owner).map_err(Error::Authority)?;
        builder.declared(outer)?;
    }
    builder.finish()
}
