use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamFreeMirCallableBindingV1 {
    pub(super) origin: MirCallableOriginV1,
    pub(super) implementation: StrongCallableDefinitionOwner,
    pub(super) semantic: MirBridgeCallableSignatureV1,
    pub(super) lowered: MirBridgeCallableSignatureV1,
    pub(super) role: MirCallableLoweringRoleV1,
}
impl ParamFreeMirCallableBindingV1 {
    pub fn try_new(
        authority: MirCallableBridgeAuthority<'_>,
        origin: MirCallableOriginV1,
        implementation: StrongCallableDefinitionOwner,
        semantic: MirBridgeCallableSignatureV1,
        lowered: MirBridgeCallableSignatureV1,
        role: MirCallableLoweringRoleV1,
    ) -> Result<Self, MirCallableBridgeError> {
        let binding = Self {
            origin,
            implementation,
            semantic,
            lowered,
            role,
        };
        authority.validate(&binding)?;
        Ok(binding)
    }
    pub const fn origin(&self) -> &MirCallableOriginV1 {
        &self.origin
    }
    pub const fn implementation(&self) -> StrongCallableDefinitionOwner {
        self.implementation
    }
    pub const fn semantic_signature(&self) -> &MirBridgeCallableSignatureV1 {
        &self.semantic
    }
    pub const fn lowered_signature(&self) -> &MirBridgeCallableSignatureV1 {
        &self.lowered
    }
    pub const fn lowering_role(&self) -> &MirCallableLoweringRoleV1 {
        &self.role
    }
}

/// Complete callable identity/signature inputs; source selection and the
/// actual body's GC effect are joined by the enclosing section producer.
#[derive(Clone, Copy)]
pub struct MirCallableBridgeAuthority<'a> {
    pub identities: &'a ValidatedIdentityGraph,
    pub foundation: &'a crate::CanonicalMirFoundation,
    pub types: &'a dyn MirTypeBridgeTypeLookupV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalMirCallableBindingsV1 {
    pub(super) entries: Vec<ParamFreeMirCallableBindingV1>,
}
impl CanonicalMirCallableBindingsV1 {
    pub fn try_new(
        mut entries: Vec<ParamFreeMirCallableBindingV1>,
    ) -> Result<Self, MirCallableBridgeError> {
        entries.sort_unstable_by_key(ParamFreeMirCallableBindingV1::implementation);
        if let Some(pair) = entries
            .windows(2)
            .find(|pair| pair[0].implementation == pair[1].implementation)
        {
            return Err(MirCallableBridgeError::DuplicateImplementation {
                implementation: pair[0].implementation,
            });
        }
        Ok(Self { entries })
    }
    pub fn entries(&self) -> &[ParamFreeMirCallableBindingV1] {
        &self.entries
    }
    pub fn into_entries(self) -> Vec<ParamFreeMirCallableBindingV1> {
        self.entries
    }
    pub fn get(
        &self,
        implementation: StrongCallableDefinitionOwner,
    ) -> Option<&ParamFreeMirCallableBindingV1> {
        self.entries
            .binary_search_by_key(
                &implementation,
                ParamFreeMirCallableBindingV1::implementation,
            )
            .ok()
            .map(|index| &self.entries[index])
    }
}
