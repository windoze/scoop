use scoop_identity::{
    CborIdentityRecord, FieldIdentityKey, FieldIdentityView, NominalDeclarationOwner,
    PersistentFieldId,
};

use super::*;
use crate::{
    AggregateStorageLayoutV1, CLayoutStorageReplayV1, DeclaredFieldStorageV1, TupleStorageLayoutV1,
};

#[derive(Clone, Copy, Debug)]
pub struct NominalLayoutFieldInputV1<'a> {
    pub field: &'a CborIdentityRecord<PersistentFieldId, FieldIdentityKey>,
    pub value: &'a ExactValueLayoutV1,
}

pub(super) fn nominal_fields<'a>(
    identity: &ExactLayoutIdentityV1,
    fields: &[NominalLayoutFieldInputV1<'a>],
    meter: &mut BudgetMeter,
) -> Result<Vec<DeclaredFieldStorageV1<'a>>, ExactLayoutReplayError> {
    let owner = nominal(identity.exact_key())?;
    nominal_fields_for_owner(identity, owner, fields, meter)
}

pub(super) fn nominal_fields_for_owner<'a>(
    identity: &ExactLayoutIdentityV1,
    owner: NominalDeclarationOwner,
    fields: &[NominalLayoutFieldInputV1<'a>],
    meter: &mut BudgetMeter,
) -> Result<Vec<DeclaredFieldStorageV1<'a>>, ExactLayoutReplayError> {
    let mut declared = reserve(fields.len(), meter)?;
    meter.charge_work(fields.len() as u64, &WirePath::root())?;
    // The storage replay allocates its placed fields and duplicate-id set.
    meter.charge_collection_slots(fields.len() as u64, &WirePath::root())?;
    meter.charge_collection_slots(fields.len() as u64, &WirePath::root())?;
    for field in fields {
        let actual = match field.field.key().view() {
            FieldIdentityView::SourceDeclared { owner, .. }
            | FieldIdentityView::SourcePropertyBacking { owner, .. }
            | FieldIdentityView::SourcePropertyDelegate { owner, .. } => owner,
            FieldIdentityView::Generated { owner, .. } => NominalDeclarationOwner::Concrete(owner),
        };
        if actual != owner {
            return Err(ExactLayoutReplayError::FieldOwner);
        }
        if field.value.identity.target() != identity.target() {
            return Err(ExactLayoutReplayError::DependencyTarget);
        }
        if let Some(storage) = field.value.value.storage().nonzero() {
            charge_scan(storage.scan(), meter)?;
        }
        declared.push(DeclaredFieldStorageV1::new(
            field.field.id(),
            &field.value.value,
        ));
    }
    Ok(declared)
}

impl ExactValueLayoutV1 {
    pub fn ordinary_struct(
        identity: ExactLayoutIdentityV1,
        interior_mutable: bool,
        fields: &[NominalLayoutFieldInputV1<'_>],
        foundation: &OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ExactLayoutReplayError> {
        require_roles(&identity, &[RepresentationRole::ManagedValue])?;
        let declared = nominal_fields(&identity, fields, meter)?;
        let aggregate = AggregateStorageLayoutV1::ordinary(identity.target(), &declared)?;
        let storage = aggregate.storage().clone();
        let representation = StructRepresentationLayoutV1 {
            policy: StructLayoutPolicyV1::Ordinary(aggregate),
            interior_mutable,
        };
        finish_value(
            identity,
            storage,
            ValueRepresentation::Struct(representation),
            foundation,
            meter,
        )
    }

    pub fn c_struct(
        identity: ExactLayoutIdentityV1,
        interior_mutable: bool,
        fields: &[NominalLayoutFieldInputV1<'_>],
        contract: &scoop_identity::CanonicalCAbiLayoutFingerprintRecord,
        foundation: &OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ExactLayoutReplayError> {
        require_roles(
            &identity,
            &[RepresentationRole::ManagedValue, RepresentationRole::CValue],
        )?;
        if contract.layout().exact_type() != identity.exact() {
            return Err(ExactLayoutReplayError::IdentityKind);
        }
        meter.charge_work(foundation.c_abi_layouts().len() as u64, &WirePath::root())?;
        if !foundation.c_abi_layouts().contains(contract) {
            return Err(ExactLayoutReplayError::MissingCLayout);
        }
        let declared = nominal_fields(&identity, fields, meter)?;
        let mut nested = reserve(fields.len(), meter)?;
        for field in fields {
            if let ValueRepresentation::Struct(StructRepresentationLayoutV1 {
                policy: StructLayoutPolicyV1::CLayout(layout),
                ..
            }) = &field.value.representation.0
            {
                nested.push(layout);
            }
        }
        meter.charge_canonical_sequence(nested.len() as u64, &WirePath::root())?;
        nested.sort_by_key(|layout| layout.contract().fingerprint());
        nested.dedup_by_key(|layout| layout.contract().fingerprint());
        let layout =
            CLayoutStorageReplayV1::replay(identity.target(), contract, &declared, &nested)?;
        let storage = layout.aggregate().storage().clone();
        let representation = StructRepresentationLayoutV1 {
            policy: StructLayoutPolicyV1::CLayout(layout),
            interior_mutable,
        };
        finish_value(
            identity,
            storage,
            ValueRepresentation::Struct(representation),
            foundation,
            meter,
        )
    }

    pub fn tuple(
        identity: ExactLayoutIdentityV1,
        elements: &[&ExactValueLayoutV1],
        foundation: &OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ExactLayoutReplayError> {
        require_roles(&identity, &[RepresentationRole::ManagedValue])?;
        let mut values = reserve(elements.len(), meter)?;
        values.extend(elements.iter().map(|value| &value.value));
        let layout = TupleStorageLayoutV1::replay(
            identity.target(),
            identity.exact_record(),
            &values,
            meter,
        )?;
        finish_value(
            identity,
            layout.storage().clone(),
            ValueRepresentation::Tuple(layout),
            foundation,
            meter,
        )
    }
}
