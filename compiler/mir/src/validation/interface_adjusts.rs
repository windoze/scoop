use super::*;
use scoop_identity::{
    CallableOdrMemberId, CallableOwner, DispatchRole, ExactTypeKey, GeneratedCallableKey,
    OdrMemberDiscriminator, OdrMemberRole, OptionalExactOwner,
};
use std::collections::HashSet;

pub(super) fn validate_interface_adjust_metadata(
    module: &Module,
) -> Result<(), MirValidationError> {
    let mut locations = HashSet::new();
    let mut functions = HashSet::new();
    for (index, adjust) in module.meta.interface_adjusts.iter().enumerate() {
        let location = MirValidationLocation::BoxingAdjust {
            adjust: index as u32,
        };
        let valid_target = match adjust.target() {
            InterfaceAdjustTarget::Local(id) => {
                (id.into_raw().into_u32() as usize) < module.functions.len()
            }
            InterfaceAdjustTarget::External(id) => {
                (id.into_raw().into_u32() as usize) < module.meta.external_callables.len()
            }
        };
        let valid_function =
            (adjust.function().into_raw().into_u32() as usize) < module.functions.len();
        let valid_interface = (adjust.interface().into_raw().into_u32() as usize)
            < module.interfaces.len()
            && (adjust.slot() as usize) < module.interfaces[adjust.interface()].methods.len();
        let valid_table = if (adjust.class().into_raw().into_u32() as usize) < module.classes.len()
        {
            let mut tables = module.classes[adjust.class()]
                .itables
                .iter()
                .filter(|table| table.interface == adjust.interface());
            let matching = tables.next().is_some_and(|table| {
                matches!(table.slots.get(adjust.slot() as usize), Some(TableSlot::Function(function)) if *function == adjust.function())
            });
            matching && tables.next().is_none()
        } else {
            false
        };
        if !valid_target || !valid_function || !valid_interface || !valid_table {
            return invalid_adjust(
                location,
                "the physical itable slot or target is invalid for the adjust",
            );
        }
        let receiver_type = module.classes[adjust.class()].physical_type(adjust.class());
        if module.functions[adjust.function()]
            .params
            .first()
            .is_none_or(|receiver| receiver.ty != receiver_type)
        {
            return invalid_adjust(
                location,
                "the adjust receiver is not the concrete object type",
            );
        }
        if !locations.insert((adjust.class(), adjust.interface(), adjust.slot())) {
            return invalid_adjust(location, "the same itable slot has multiple adjusts");
        }
        if !functions.insert(adjust.function()) {
            return invalid_adjust(
                location,
                "the same function implements multiple interface adjusts",
            );
        }
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
        let (owner, expected_receiver) = match identity.callable_record().key() {
            GeneratedCallableKey::BoxingAdjust {
                slot,
                payload,
                interface: key_interface,
            } => {
                let boxed = module
                    .meta
                    .boxed_types
                    .iter()
                    .find(|boxed| boxed.class() == adjust.class())
                    .ok_or_else(|| {
                        adjust_error(
                            location,
                            "the boxing adjust class is not a materialized value box",
                        )
                    })?;
                let owner = module
                    .meta
                    .source_exact_types
                    .get(boxed.payload())
                    .ok_or_else(|| {
                        adjust_error(location, "the boxed payload has no source exact identity")
                    })?;
                if *slot != identity.slot_record().id()
                    || *payload != owner.identity_record().id()
                    || *key_interface != interface.identity_record().id()
                {
                    return invalid_adjust(
                        location,
                        "the generated callable key does not match its slot and exact types",
                    );
                }
                let receiver = InterfaceAdjustIdentity::boxed_receiver(*payload)
                    .map_err(|_| adjust_error(location, "the boxing adjust receiver is invalid"))?;
                (owner, receiver)
            }
            GeneratedCallableKey::DispatchAdjust {
                slot, implementor, ..
            } => {
                let owner = module
                    .meta
                    .source_exact_types
                    .get(&receiver_type)
                    .ok_or_else(|| {
                        adjust_error(
                            location,
                            "the reference adjust owner has no source exact identity",
                        )
                    })?;
                if *slot != identity.slot_record().id()
                    || *implementor != owner.identity_record().id()
                {
                    return invalid_adjust(
                        location,
                        "the generated callable key does not match its slot and exact types",
                    );
                }
                (owner, *implementor)
            }
            _ => {
                return invalid_adjust(
                    location,
                    "the generated callable is not an interface adjust",
                );
            }
        };
        let expected_root = ExactOwnerRoot::for_member(
            owner.identity_record(),
            owner.nominal_specialization(),
            OdrMemberRole::DispatchAdapter,
            OdrMemberDiscriminator::GeneratedCallable(identity.callable_record().id()),
        )
        .map_err(|_| adjust_error(location, "the interface adjust has an invalid owner root"))?;
        if identity.root() != &expected_root {
            return invalid_adjust(location, "the interface adjust has a different owner root");
        }
        let expected_subject = match expected_root.member_record() {
            Some(member) => CallableSignatureSubject::odr(
                CallableOdrMemberId::from_key(member.key()).map_err(|_| {
                    adjust_error(
                        location,
                        "the interface adjust has a non-callable ODR member",
                    )
                })?,
            ),
            None => CallableSignatureSubject::strong(CallableOwner::Generated(
                identity.callable_record().id(),
            )),
        };
        let signature = identity.signature_record();
        if signature.subject() != expected_subject {
            return invalid_adjust(
                location,
                "the signature subject does not match the owner root",
            );
        }
        if signature.signature().receiver() != OptionalExactOwner::Present(expected_receiver) {
            return invalid_adjust(
                location,
                "the signature receiver does not match the concrete object type",
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
                if !module.meta.interface_adjusts.iter().any(|adjust| {
                    adjust.class() == boxed.class()
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
        adjust: module.meta.interface_adjusts.len() as u32,
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
