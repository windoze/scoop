use std::collections::HashSet;

use scoop_identity::{
    CallableOdrMemberId, CallableOwner, DispatchRole, ExactTypeKey, FieldIdentityKey,
    GeneratedCallableKey, GeneratedNominalKey, OdrMemberDiscriminator, OdrMemberRole,
    OptionalExactOwner, SpecializationKey,
};

use super::*;
use crate::validation::metadata::source_exact_type;

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
        if source_exact_type(module, boxed.payload()) != Some(*payload) {
            return invalid(
                location,
                "boxed payload does not match the source exact-type relation",
            );
        }
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

pub(super) fn validate_boxing_adjust_metadata(module: &Module) -> Result<(), MirValidationError> {
    let mut locations = HashSet::new();
    let mut functions = HashSet::new();
    for (index, adjust) in module.meta.boxing_adjusts.iter().enumerate() {
        let location = MirValidationLocation::BoxingAdjust {
            adjust: index as u32,
        };
        if BoxingAdjust::checked(
            &module.functions,
            &module.classes,
            &module.interfaces,
            adjust.location(),
            adjust.target(),
            adjust.identity().clone(),
        )
        .is_none()
        {
            return invalid_adjust(
                location,
                "the physical itable slot or target is invalid for the adjust",
            );
        }
        if module.functions[adjust.function()]
            .params
            .first()
            .is_none_or(|receiver| receiver.ty != Type::Interface(adjust.interface()))
        {
            return invalid_adjust(
                location,
                "the adjust receiver does not retain its interface type",
            );
        }
        if !locations.insert((adjust.boxed(), adjust.interface(), adjust.slot())) {
            return invalid_adjust(location, "the same boxed itable slot has multiple adjusts");
        }
        if !functions.insert(adjust.function()) {
            return invalid_adjust(
                location,
                "the same function implements multiple boxing adjusts",
            );
        }
        let Some(boxed) = module
            .meta
            .boxed_types
            .iter()
            .find(|boxed| boxed.class() == adjust.boxed())
        else {
            return invalid_adjust(location, "the adjust class is not a materialized value box");
        };
        let Some(payload) = module.meta.source_exact_types.get(boxed.payload()) else {
            return invalid_adjust(location, "the boxed payload has no source exact identity");
        };
        let Some(interface) = module
            .meta
            .source_exact_types
            .get(&Type::Interface(adjust.interface()))
        else {
            return invalid_adjust(
                location,
                "the itable interface has no source exact identity",
            );
        };
        if !matches!(
            interface.identity_record().key(),
            ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. }
        ) {
            return invalid_adjust(
                location,
                "the itable interface is not an exact nominal type",
            );
        }
        let identity = adjust.identity();
        if !matches!(
            identity.slot_record().key().role(),
            DispatchRole::InterfaceMethod
                | DispatchRole::PropertyGetter
                | DispatchRole::PropertySetter
        ) {
            return invalid_adjust(location, "the adjust does not implement an interface slot");
        }
        let GeneratedCallableKey::BoxingAdjust {
            slot,
            payload: key_payload,
            interface: key_interface,
        } = identity.callable_record().key()
        else {
            return invalid_adjust(location, "the generated callable is not a boxing adjust");
        };
        if *slot != identity.slot_record().id()
            || *key_payload != payload.identity_record().id()
            || *key_interface != interface.identity_record().id()
        {
            return invalid_adjust(
                location,
                "the generated callable key does not match its slot and exact types",
            );
        }
        let expected_root = ExactOwnerRoot::for_member(
            payload.identity_record(),
            payload.nominal_specialization(),
            OdrMemberRole::DispatchAdapter,
            OdrMemberDiscriminator::GeneratedCallable(identity.callable_record().id()),
        )
        .map_err(|_| adjust_error(location, "the boxing adjust has an invalid payload root"))?;
        if identity.root() != &expected_root {
            return invalid_adjust(location, "the boxing adjust has a different payload root");
        }
        let expected_subject = match expected_root.member_record() {
            Some(member) => {
                CallableSignatureSubject::odr(CallableOdrMemberId::from_key(member.key()).map_err(
                    |_| adjust_error(location, "the boxing adjust has a non-callable ODR member"),
                )?)
            }
            None => CallableSignatureSubject::strong(CallableOwner::Generated(
                identity.callable_record().id(),
            )),
        };
        let signature = identity.signature_record();
        if signature.subject() != expected_subject {
            return invalid_adjust(
                location,
                "the signature subject does not match the payload root",
            );
        }
        if signature.signature().receiver()
            != OptionalExactOwner::Present(interface.identity_record().id())
        {
            return invalid_adjust(
                location,
                "the signature receiver does not match the interface",
            );
        }
        if signature
            .signature()
            .parameters()
            .iter()
            .copied()
            .chain(std::iter::once(signature.signature().result()))
            .any(|exact| {
                module
                    .meta
                    .source_exact_types
                    .get_by_identity(exact)
                    .is_none()
            })
        {
            return invalid_adjust(location, "the signature references an unknown exact type");
        }
    }

    for boxed in &module.meta.boxed_types {
        let class = &module.classes[boxed.class()];
        for table in &class.itables {
            if !class.interfaces.contains(&table.interface) {
                return invalid_adjust(
                    next_adjust_location(module),
                    "a boxed itable is absent from the class interface list",
                );
            }
            for (slot, entry) in table.slots.iter().enumerate() {
                let TableSlot::Function(function) = entry else {
                    return invalid_adjust(
                        next_adjust_location(module),
                        "a boxed itable slot is not implemented by a generated function",
                    );
                };
                let Ok(slot) = u32::try_from(slot) else {
                    return invalid_adjust(
                        next_adjust_location(module),
                        "a boxed itable has more slots than the identity model can represent",
                    );
                };
                if !module.meta.boxing_adjusts.iter().any(|adjust| {
                    adjust.boxed() == boxed.class()
                        && adjust.interface() == table.interface
                        && adjust.slot() == slot
                        && adjust.function() == *function
                }) {
                    return invalid_adjust(
                        next_adjust_location(module),
                        "a boxed itable slot has no persistent adjust identity",
                    );
                }
            }
        }
    }
    Ok(())
}

fn next_adjust_location(module: &Module) -> MirValidationLocation {
    MirValidationLocation::BoxingAdjust {
        adjust: module.meta.boxing_adjusts.len() as u32,
    }
}

fn invalid_adjust<T>(
    location: MirValidationLocation,
    reason: &'static str,
) -> Result<T, MirValidationError> {
    Err(adjust_error(location, reason))
}

fn adjust_error(location: MirValidationLocation, reason: &'static str) -> MirValidationError {
    MirValidationError {
        location,
        kind: MirValidationErrorKind::InvalidBoxingAdjust { reason },
    }
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
