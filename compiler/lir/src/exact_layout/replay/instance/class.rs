use scoop_identity::{CborIdentityRecord, NominalDeclarationOwner};

use super::*;
use crate::{ClassBaseStorageV1, ClassStorageLayoutV1};

#[derive(Clone, Copy, Debug)]
pub enum ClassLayoutBaseV1<'a> {
    NoBase,
    Base(&'a ExactInstanceLayoutV1),
}

impl ExactInstanceLayoutV1 {
    pub fn throwable(
        identity: ExactLayoutIdentityV1,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, ExactLayoutReplayError> {
        require_roles(&identity, &[RepresentationRole::ManagedObject])?;
        nominal(identity.exact_key())?;
        let layout =
            ClassStorageLayoutV1::throwable(identity.target(), identity.layout_key().clone())?;
        finish_instance(
            identity,
            layout.shape().clone(),
            InstanceRepresentation::Throwable(Box::new(layout)),
            ScanRole::ManagedObject,
            foundation,
        )
    }

    pub fn class(
        identity: ExactLayoutIdentityV1,
        base: ClassLayoutBaseV1<'_>,
        fields: &[NominalLayoutFieldInputV1<'_>],
        foundation: &ConeLirFoundation,
    ) -> Result<Self, ExactLayoutReplayError> {
        let owner = nominal(identity.exact_key())?;
        replay_class(identity, owner, base, fields, foundation)
    }

    /// Preserves the source object exact while checking its separately
    /// identified backing class as the declaration owner of stored fields.
    pub fn object(
        identity: ExactLayoutIdentityV1,
        backing: &CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>,
        base: ClassLayoutBaseV1<'_>,
        fields: &[NominalLayoutFieldInputV1<'_>],
        foundation: &ConeLirFoundation,
    ) -> Result<Self, ExactLayoutReplayError> {
        nominal(identity.exact_key())?;
        let matches = match (backing.key(), identity.exact_key()) {
            (GeneratedNominalKey::ObjectBackingClass { object }, ExactTypeKey::Nominal(actual)) => {
                object == actual
            }
            (
                GeneratedNominalKey::GenericObjectBackingClass { object },
                ExactTypeKey::NominalApplication { origin, .. },
            ) => object == origin,
            _ => false,
        };
        if !matches {
            return Err(ExactLayoutReplayError::ObjectBackingIdentity);
        }
        replay_class(
            identity,
            NominalDeclarationOwner::Concrete(backing.id()),
            base,
            fields,
            foundation,
        )
    }
}

fn replay_class(
    identity: ExactLayoutIdentityV1,
    owner: NominalDeclarationOwner,
    base: ClassLayoutBaseV1<'_>,
    fields: &[NominalLayoutFieldInputV1<'_>],
    foundation: &ConeLirFoundation,
) -> Result<ExactInstanceLayoutV1, ExactLayoutReplayError> {
    require_roles(&identity, &[RepresentationRole::ManagedObject])?;
    let declared = aggregate::nominal_fields_for_owner(&identity, owner, fields)?;
    let base = match base {
        ClassLayoutBaseV1::NoBase => ClassBaseStorageV1::NoBase,
        ClassLayoutBaseV1::Base(base) => {
            if base.identity.target() != identity.target() {
                return Err(ExactLayoutReplayError::DependencyTarget);
            }
            let layout = match &base.representation.0 {
                InstanceRepresentation::ClassObject(layout)
                | InstanceRepresentation::Throwable(layout) => layout,
                _ => return Err(ExactLayoutReplayError::BaseKind),
            };

            // Reserve the inherited projection and duplicate-id set before
            // ClassStorageLayoutV1 clones either collection.

            ClassBaseStorageV1::Base(layout)
        }
    };

    let layout = ClassStorageLayoutV1::replay(
        identity.target(),
        identity.layout_key().clone(),
        base,
        &declared,
    )?;
    finish_instance(
        identity,
        layout.shape().clone(),
        InstanceRepresentation::ClassObject(Box::new(layout)),
        ScanRole::ManagedObject,
        foundation,
    )
}
