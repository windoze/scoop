use super::*;

/// An export whose exact origin and ordered representation have been checked
/// against the provider's identity graph and MIR generated identity foundation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamFreeMirTypeExportV1 {
    exact: PersistentExactTypeId,
    origin: MirTypeOriginV1,
    facts: MirTypeFactsV1,
    representation: MirTypeRepresentationV1,
    base_and_interfaces: MirBaseAndInterfacesV1,
}
impl ParamFreeMirTypeExportV1 {
    pub fn try_new(
        authority: MirTypeBridgeAuthority<'_>,
        exact: PersistentExactTypeId,
        origin: MirTypeOriginV1,
        facts: MirTypeFactsV1,
        representation: MirTypeRepresentationV1,
        base_and_interfaces: MirBaseAndInterfacesV1,
    ) -> Result<Self, MirTypeBridgeError> {
        let record = Self {
            exact,
            origin,
            facts,
            representation,
            base_and_interfaces,
        };
        authority.validate(&record)?;
        Ok(record)
    }
    pub const fn exact(&self) -> PersistentExactTypeId {
        self.exact
    }
    pub const fn origin(&self) -> &MirTypeOriginV1 {
        &self.origin
    }
    pub const fn facts(&self) -> MirTypeFactsV1 {
        self.facts
    }
    pub const fn representation(&self) -> &MirTypeRepresentationV1 {
        &self.representation
    }
    pub const fn base_and_interfaces(&self) -> &MirBaseAndInterfacesV1 {
        &self.base_and_interfaces
    }
}

/// Explicit, read-only semantic authority; callers cannot replace absent
/// generated keys with coincidentally equal storage shapes.
#[derive(Clone, Copy)]
pub struct MirTypeBridgeAuthority<'a> {
    pub identities: &'a ValidatedIdentityGraph,
    pub foundation: &'a crate::CanonicalMirFoundation,
}
