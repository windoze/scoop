use scoop_identity::{ExactTypeKey, NominalDeclarationOwner, PersistentExactTypeId};

use super::CanonicalHirFoundation;

pub(super) fn nominal_owner(key: &ExactTypeKey) -> Option<NominalDeclarationOwner> {
    match key {
        ExactTypeKey::Nominal(owner) => Some(NominalDeclarationOwner::Concrete(*owner)),
        ExactTypeKey::NominalApplication { origin, .. } => {
            Some(NominalDeclarationOwner::GenericTemplate(*origin))
        }
        _ => None,
    }
}

impl CanonicalHirFoundation {
    pub(crate) fn release_hook_owner(
        &self,
        exact: PersistentExactTypeId,
    ) -> Option<NominalDeclarationOwner> {
        nominal_owner(self.exact_type_by_bytes(exact.as_array())?.1)
    }
}
