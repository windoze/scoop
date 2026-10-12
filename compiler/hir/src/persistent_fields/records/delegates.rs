use super::*;

pub(in crate::persistent_fields) fn interface_delegate_record(
    owner: ClassId,
    field: ClassFieldId,
    ordinal: u32,
    nominal_identities: &HirNominalIdentities,
) -> Result<FieldRecord, HirFieldIdentityError> {
    let key = match &nominal_identities[owner] {
        crate::HirNominalIdentity::Source(owner) => {
            FieldIdentityKey::source_interface_delegate(owner.declaration(), ordinal)
        }
        crate::HirNominalIdentity::Generated(owner) => {
            FieldIdentityKey::object_interface_delegate(owner.key(), ordinal)
        }
    }
    .map_err(|error| HirFieldIdentityError::InvalidClassFieldIdentity {
        class: raw_index(owner),
        field: raw_index(field),
        error,
    })?;
    CborIdentityRecord::from_key(key).map_err(|error| {
        HirFieldIdentityError::InvalidClassFieldIdentity {
            class: raw_index(owner),
            field: raw_index(field),
            error,
        }
    })
}
