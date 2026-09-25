use scoop_identity::{CborIdentityRecord, NominalDeclarationOwner};

use super::*;
use crate::{ClassBaseStorageV1, ClassStorageLayoutV1};

#[derive(Clone, Copy, Debug)]
pub enum ClassLayoutBaseV1<'a> {
    NoBase,
    Base(&'a ExactInstanceLayoutV1),
}

impl ExactInstanceLayoutV1 {
    pub fn class(
        identity: ExactLayoutIdentityV1,
        base: ClassLayoutBaseV1<'_>,
        fields: &[NominalLayoutFieldInputV1<'_>],
        foundation: &OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ExactLayoutReplayError> {
        let owner = nominal(identity.exact_key())?;
        replay_class(identity, owner, base, fields, foundation, meter)
    }

    /// Preserves the source object exact while checking its separately
    /// identified backing class as the declaration owner of stored fields.
    pub fn object(
        identity: ExactLayoutIdentityV1,
        backing: &CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>,
        base: ClassLayoutBaseV1<'_>,
        fields: &[NominalLayoutFieldInputV1<'_>],
        foundation: &OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ExactLayoutReplayError> {
        nominal(identity.exact_key())?;
        let GeneratedNominalKey::ObjectBackingClass { object } = backing.key() else {
            return Err(ExactLayoutReplayError::ObjectBackingIdentity);
        };
        if identity.exact_key() != &ExactTypeKey::Nominal(*object) {
            return Err(ExactLayoutReplayError::ObjectBackingIdentity);
        }
        replay_class(
            identity,
            NominalDeclarationOwner::Concrete(backing.id()),
            base,
            fields,
            foundation,
            meter,
        )
    }
}

fn replay_class(
    identity: ExactLayoutIdentityV1,
    owner: NominalDeclarationOwner,
    base: ClassLayoutBaseV1<'_>,
    fields: &[NominalLayoutFieldInputV1<'_>],
    foundation: &OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<ExactInstanceLayoutV1, ExactLayoutReplayError> {
    require_roles(&identity, &[RepresentationRole::ManagedObject])?;
    let declared = aggregate::nominal_fields_for_owner(&identity, owner, fields, meter)?;
    let base = match base {
        ClassLayoutBaseV1::NoBase => ClassBaseStorageV1::NoBase,
        ClassLayoutBaseV1::Base(base) => {
            if base.identity.target() != identity.target() {
                return Err(ExactLayoutReplayError::DependencyTarget);
            }
            let InstanceRepresentation::ClassObject(layout) = &base.representation.0 else {
                return Err(ExactLayoutReplayError::BaseKind);
            };
            meter.check_semantic_depth(layout.inheritance_depth() as u64 + 1, &WirePath::root())?;
            meter.charge_work(layout.inheritance_depth() as u64, &WirePath::root())?;
            meter.charge_collection_slots(
                layout.inheritance_depth() as u64 + 1,
                &WirePath::root(),
            )?;
            // Reserve the inherited projection and duplicate-id set before
            // ClassStorageLayoutV1 clones either collection.
            meter.charge_collection_slots(
                layout.complete_fields().len() as u64,
                &WirePath::root(),
            )?;
            meter.charge_collection_slots(
                layout.complete_fields().len() as u64,
                &WirePath::root(),
            )?;
            ClassBaseStorageV1::Base(layout)
        }
    };
    meter.charge_collection_slots(fields.len() as u64, &WirePath::root())?;
    let layout = ClassStorageLayoutV1::replay(
        identity.target(),
        identity.layout_key().clone(),
        base,
        &declared,
    )?;
    finish_instance(
        identity,
        layout.shape().clone(),
        InstanceRepresentation::ClassObject(layout),
        ScanRole::ManagedObject,
        foundation,
        meter,
    )
}
