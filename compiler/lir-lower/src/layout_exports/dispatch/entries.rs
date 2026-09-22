use super::*;

pub(super) fn project<'a>(
    slots: &[mir::MirDispatchEntryV1],
    layouts: &'a lookup::Layouts<'_>,
    callables: &'a lir::CanonicalExactCallableAbiExportsV1,
    dependencies: &'a [&'a lir::CanonicalExactCallableAbiExportsV1],
    meter: &mut BudgetMeter,
) -> Result<Vec<lir::ExactDispatchEntryInputV1<'a>>, Error> {
    let mut entries = reserve(slots.len(), meter)?;
    for slot in slots {
        let implementation = implementation(slot.implementation());
        let abi = lookup::callable(implementation.target(), callables, dependencies, meter)?;
        let signature = slot.signature();
        let receiver = match implementation.receiver_adaptation() {
            lir::ExactDispatchReceiverAdaptationV1::Identity => None,
            lir::ExactDispatchReceiverAdaptationV1::ReferenceDispatch => signature
                .exact()
                .receiver()
                .into_option()
                .map(|exact| layouts.value(exact, meter))
                .transpose()?,
        };
        meter.charge_collection_slots(
            signature.exact().parameters().len() as u64,
            &WirePath::root(),
        )?;
        entries.push(lir::ExactDispatchEntryInputV1 {
            position: lir::ExactDispatchPositionV1::from_u32(slot.position().get()),
            slot: slot.slot(),
            slot_signature: lir::ExactDispatchSlotSignatureV1::new(
                signature.exact().clone(),
                match signature.gc_effect() {
                    mir::GcEffect::Managed => scoop_identity::GcEffect::Managed,
                    mir::GcEffect::NoGc => scoop_identity::GcEffect::NoGc,
                },
            ),
            implementation,
            abi,
            slot_receiver_layout: receiver,
        });
    }
    Ok(entries)
}

fn receiver(value: mir::MirDispatchReceiverAdaptationV1) -> lir::ExactDispatchReceiverAdaptationV1 {
    match value {
        mir::MirDispatchReceiverAdaptationV1::Identity => {
            lir::ExactDispatchReceiverAdaptationV1::Identity
        }
        mir::MirDispatchReceiverAdaptationV1::ReferenceDispatch => {
            lir::ExactDispatchReceiverAdaptationV1::ReferenceDispatch
        }
    }
}

fn implementation(value: mir::MirDispatchImplementationV1) -> lir::ExactDispatchImplementationV1 {
    match value {
        mir::MirDispatchImplementationV1::AbstractObligation {
            declaration,
            trap_target,
            receiver: adaptation,
        } => lir::ExactDispatchImplementationV1::AbstractObligation {
            declaration,
            trap_target,
            receiver: receiver(adaptation),
        },
        mir::MirDispatchImplementationV1::DirectStrongTarget {
            target,
            receiver: adaptation,
        } => lir::ExactDispatchImplementationV1::DirectStrongTarget {
            target,
            receiver: receiver(adaptation),
        },
        mir::MirDispatchImplementationV1::InterfaceDefaultTarget {
            target,
            receiver: adaptation,
        } => lir::ExactDispatchImplementationV1::InterfaceDefaultTarget {
            target,
            receiver: receiver(adaptation),
        },
        mir::MirDispatchImplementationV1::AdjustThunkTarget(target) => {
            lir::ExactDispatchImplementationV1::AdjustThunkTarget(target)
        }
    }
}
