use super::*;
use scoop_identity::{PersistentExtensionPropertyId, PropertyOwner};

pub(super) fn declare(
    identities: &ValidatedIdentityGraph,
    pending: &mut PendingIdentityValidation,
    site: SourceDeclarationSite,
    name: &str,
    receiver: Option<SignatureTypeKey>,
    role: AccessorRole,
) -> PersistentPropertyAccessorId {
    let name = CanonicalIdentifier::new(name).unwrap();
    let owner = if let Some(receiver) = receiver {
        let property = CborIdentityRecord::<PersistentExtensionPropertyId, _>::from_key(
            SourceDeclarationKey::extension_property(site, name, 0, receiver),
        )
        .unwrap();
        let id = property.id();
        if identities
            .canonical_key::<_, SourceDeclarationKey>(id)
            .is_err()
        {
            pending
                .register_external_canonical_authority(property)
                .unwrap();
        }
        PropertyOwner::ExtensionProperty(id)
    } else {
        let property = CborIdentityRecord::<PersistentPropertyId, _>::from_key(
            SourceDeclarationKey::property(site, name),
        )
        .unwrap();
        let id = property.id();
        if identities
            .canonical_key::<_, SourceDeclarationKey>(id)
            .is_err()
        {
            pending
                .register_external_canonical_authority(property)
                .unwrap();
        }
        PropertyOwner::Property(id)
    };
    let accessor = CborIdentityRecord::<PersistentPropertyAccessorId, _>::from_key(
        PropertyAccessorKey::new(owner, role),
    )
    .unwrap();
    let id = accessor.id();
    pending
        .register_external_canonical_authority(accessor)
        .unwrap();
    id
}
