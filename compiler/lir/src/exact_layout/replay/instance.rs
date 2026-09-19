use scoop_identity::{GeneratedNominalKey, PersistentTypeId};

use super::*;
use crate::{
    ArrayElementStorageV1, ClassBaseStorageV1, ClassStorageLayoutV1, FieldStorageKindV1,
    TypeInstanceShapeV1,
};

#[derive(Clone, Copy, Debug)]
pub enum ClassLayoutBaseV1<'a> {
    NoBase,
    Base(&'a ExactInstanceLayoutV1),
}

fn finish_instance(
    identity: ExactLayoutIdentityV1,
    shape: TypeInstanceShapeV1,
    representation: InstanceRepresentation,
    role: ScanRole,
    foundation: &OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<ExactInstanceLayoutV1, ExactLayoutReplayError> {
    let scan = scan_binding(&identity, role, foundation, meter)?;
    Ok(ExactInstanceLayoutV1 {
        identity,
        shape,
        representation: InstanceRepresentationV1(representation),
        scan,
    })
}

impl ExactInstanceLayoutV1 {
    pub fn class(
        identity: ExactLayoutIdentityV1,
        base: ClassLayoutBaseV1<'_>,
        fields: &[NominalLayoutFieldInputV1<'_>],
        foundation: &OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ExactLayoutReplayError> {
        require_roles(&identity, &[RepresentationRole::ManagedObject])?;
        let declared = aggregate::nominal_fields(&identity, fields, meter)?;
        let base = match base {
            ClassLayoutBaseV1::NoBase => ClassBaseStorageV1::NoBase,
            ClassLayoutBaseV1::Base(base) => {
                if base.identity.target() != identity.target() {
                    return Err(ExactLayoutReplayError::DependencyTarget);
                }
                let InstanceRepresentation::ClassObject(layout) = &base.representation.0 else {
                    return Err(ExactLayoutReplayError::BaseKind);
                };
                meter.check_semantic_depth(
                    layout.inheritance_depth() as u64 + 1,
                    &WirePath::root(),
                )?;
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
                for field in layout.complete_fields() {
                    if let FieldStorageKindV1::Stored { layout, .. } = field.storage().kind() {
                        charge_scan(layout.storage().scan(), meter)?;
                    }
                }
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
        for field in layout.complete_fields() {
            if let FieldStorageKindV1::Stored { layout, .. } = field.storage().kind() {
                charge_scan(layout.storage().scan(), meter)?;
            }
        }
        finish_instance(
            identity,
            layout.shape().clone(),
            InstanceRepresentation::ClassObject(layout),
            ScanRole::ManagedObject,
            foundation,
            meter,
        )
    }

    pub fn boxed_payload(
        identity: ExactLayoutIdentityV1,
        payload: &ExactValueLayoutV1,
        foundation: &OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ExactLayoutReplayError> {
        require_roles(&identity, &[RepresentationRole::ManagedObject])?;
        let expected = PersistentTypeId::from_generated_key(&GeneratedNominalKey::BoxedValue {
            payload: payload.identity.exact(),
        })
        .map_err(ExactLayoutReplayError::GeneratedNominal)?;
        if identity.exact_key() != &ExactTypeKey::Nominal(expected) {
            return Err(ExactLayoutReplayError::BoxPayloadIdentity);
        }
        if payload.identity.target() != identity.target() {
            return Err(ExactLayoutReplayError::DependencyTarget);
        }
        if let Some(storage) = payload.value.storage().nonzero() {
            charge_scan(storage.scan(), meter)?;
            charge_scan(storage.scan(), meter)?;
        }
        let shape =
            TypeInstanceShapeV1::boxed_value(identity.target(), payload.value.storage().clone())?;
        finish_instance(
            identity,
            shape,
            InstanceRepresentation::BoxedPayload(payload.value.clone()),
            ScanRole::ManagedObject,
            foundation,
            meter,
        )
    }

    /// Replays array geometry. The complete section must join the application
    /// origin to the trusted-core Array or MutableArray binding.
    pub fn inline_array(
        identity: ExactLayoutIdentityV1,
        element: &ExactValueLayoutV1,
        foundation: &OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ExactLayoutReplayError> {
        require_roles(&identity, &[RepresentationRole::ManagedObject])?;
        let ExactTypeKey::NominalApplication { arguments, .. } = identity.exact_key() else {
            return Err(ExactLayoutReplayError::ArrayElementIdentity);
        };
        if arguments.as_slice() != [element.identity.exact()] {
            return Err(ExactLayoutReplayError::ArrayElementIdentity);
        }
        if element.identity.target() != identity.target() {
            return Err(ExactLayoutReplayError::DependencyTarget);
        }
        if let Some(storage) = element.value.storage().nonzero() {
            charge_scan(storage.scan(), meter)?;
            charge_scan(storage.scan(), meter)?;
        }
        let storage = ArrayElementStorageV1::from_value(element.value.storage());
        let shape = TypeInstanceShapeV1::inline_array(identity.target(), storage.clone())?;
        finish_instance(
            identity,
            shape,
            InstanceRepresentation::InlineArray {
                element: element.value.clone(),
                storage,
            },
            ScanRole::ArrayElement,
            foundation,
            meter,
        )
    }

    /// Replays byte storage. The complete section must join the exact identity
    /// to the trusted-core String binding.
    pub fn inline_bytes(
        identity: ExactLayoutIdentityV1,
        foundation: &OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ExactLayoutReplayError> {
        require_roles(&identity, &[RepresentationRole::ManagedObject])?;
        if is_unit(identity.exact_key()) {
            return Err(ExactLayoutReplayError::IdentityKind);
        }
        if !matches!(identity.exact_key(), ExactTypeKey::Nominal(_)) {
            return Err(ExactLayoutReplayError::IdentityKind);
        }
        let shape = TypeInstanceShapeV1::inline_bytes(identity.target())?;
        finish_instance(
            identity,
            shape,
            InstanceRepresentation::InlineBytes,
            ScanRole::ManagedObject,
            foundation,
            meter,
        )
    }

    pub fn abstract_reference(
        identity: ExactLayoutIdentityV1,
        foundation: &OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ExactLayoutReplayError> {
        require_roles(&identity, &[RepresentationRole::ManagedObject])?;
        if is_unit(identity.exact_key()) {
            return Err(ExactLayoutReplayError::IdentityKind);
        }
        if !matches!(
            identity.exact_key(),
            ExactTypeKey::Nominal(_)
                | ExactTypeKey::NominalApplication { .. }
                | ExactTypeKey::Function { .. }
        ) {
            return Err(ExactLayoutReplayError::IdentityKind);
        }
        finish_instance(
            identity,
            TypeInstanceShapeV1::abstract_ref(),
            InstanceRepresentation::AbstractReference,
            ScanRole::ManagedObject,
            foundation,
            meter,
        )
    }
}
