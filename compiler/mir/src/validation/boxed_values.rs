use std::collections::HashSet;

use scoop_identity::{
    FieldIdentityKey, GeneratedNominalKey, OdrMemberDiscriminator, OdrMemberRole, SpecializationKey,
};

use super::*;

pub(super) fn validate_boxed_value_metadata(module: &Module) -> Result<(), MirValidationError> {
    let mut payloads = HashSet::new();
    let mut classes = HashSet::new();
    for (index, boxed) in module.meta.boxed_types.iter().enumerate() {
        let location = MirValidationLocation::BoxedValue {
            boxed: index as u32,
        };
        let identity = boxed.identity();
        let GeneratedNominalKey::BoxedValue { payload } = identity.generated_type_record().key()
        else {
            return invalid(location, "the generated nominal is not a value box");
        };
        if !payloads.insert(*payload) {
            return invalid(location, "the same exact payload is boxed more than once");
        }
        if !classes.insert(boxed.class()) {
            return invalid(
                location,
                "the same class is claimed by more than one value box",
            );
        }
        let expected_field = FieldIdentityKey::box_payload(identity.generated_type_record().key())
            .map_err(|_| error(location, "the generated payload field is invalid"))?;
        if identity.payload_field_record().key() != &expected_field {
            return invalid(
                location,
                "the payload field does not belong to the generated box",
            );
        }
        validate_root(location, *payload, identity)?;
        validate_class(module, location, boxed)?;
    }
    Ok(())
}

fn validate_root(
    location: MirValidationLocation,
    payload: scoop_identity::PersistentExactTypeId,
    identity: &BoxedValueIdentity,
) -> Result<(), MirValidationError> {
    match identity.root() {
        ExactOwnerRoot::SourceNominal(_) => Ok(()),
        ExactOwnerRoot::NominalApplication(root) => validate_member(
            location,
            root.group(),
            root.member_record(),
            identity.generated_type_record().id(),
        ),
        ExactOwnerRoot::Structural(root) => {
            if root.group_record().key()
                != &(SpecializationKey::StructuralType {
                    exact_type: payload,
                })
            {
                return invalid(
                    location,
                    "the structural ODR group has a different exact payload",
                );
            }
            validate_member(
                location,
                root.group_record().id(),
                root.member_record(),
                identity.generated_type_record().id(),
            )
        }
    }
}

fn validate_member(
    location: MirValidationLocation,
    group: scoop_identity::OdrGroupId,
    member: &scoop_identity::CborIdentityRecord<
        scoop_identity::OdrMemberId,
        scoop_identity::OdrMemberKey,
    >,
    generated_type: scoop_identity::PersistentTypeId,
) -> Result<(), MirValidationError> {
    let member = member.key();
    if member.group() != group
        || member.role() != OdrMemberRole::GeneratedNominal
        || member.discriminator() != &OdrMemberDiscriminator::GeneratedNominal(generated_type)
    {
        return invalid(
            location,
            "the generated box is not the nominal member of its ODR group",
        );
    }
    Ok(())
}

fn validate_class(
    module: &Module,
    location: MirValidationLocation,
    boxed: &BoxedType,
) -> Result<(), MirValidationError> {
    let class_index = boxed.class().into_raw().into_u32() as usize;
    if class_index >= module.classes.len() {
        return invalid(location, "the generated box class does not exist");
    }
    let class = &module.classes[boxed.class()];
    if class.modifier != ClassModifier::Final
        || !class.type_arguments.is_empty()
        || !class.vtable.is_empty()
    {
        return invalid(location, "the generated box has an invalid class shape");
    }
    let ClassRepresentation::Declared { fields, base_class } = &class.representation else {
        return invalid(
            location,
            "the generated box must have a declared representation",
        );
    };
    if base_class.is_some()
        || fields.len() != 1
        || fields[0].name != "value"
        || fields[0].ty != *boxed.payload()
    {
        return invalid(location, "the generated box has an invalid payload field");
    }
    Ok(())
}

fn invalid<T>(
    location: MirValidationLocation,
    reason: &'static str,
) -> Result<T, MirValidationError> {
    Err(error(location, reason))
}

fn error(location: MirValidationLocation, reason: &'static str) -> MirValidationError {
    MirValidationError {
        location,
        kind: MirValidationErrorKind::InvalidBoxedValue { reason },
    }
}
