use super::*;

pub(super) fn project<'a>(
    slots: &[mir::MirDispatchEntryV1],
    abis: &lookup::Abis<'a>,
    meter: &mut BudgetMeter,
) -> Result<Vec<lir::ExactDispatchEntryInputV1<'a>>, Error> {
    let path = WirePath::root();
    meter.check_table_entries(slots.len() as u64, &path)?;
    let mut entries = Vec::new();
    meter.try_reserve_collection_slots(&mut entries, slots.len(), &path)?;
    for slot in slots {
        let implementation = implementation(slot.implementation());
        let abi = abis.callable(implementation.target(), meter)?;
        let signature = slot.signature();
        let receiver = match implementation.receiver_adaptation() {
            lir::ExactDispatchReceiverAdaptationV1::Identity => None,
            lir::ExactDispatchReceiverAdaptationV1::ReferenceDispatch => signature
                .exact()
                .receiver()
                .into_option()
                .map(|exact| abis.value(exact, meter))
                .transpose()?,
        };
        let count = signature.exact().parameters().len() as u64;
        meter.charge_work(count + 1, &path)?;
        meter.charge_edges(count + 2, &path)?;
        meter.charge_collection_slots(count, &path)?;
        meter.charge_owned_bytes(count.saturating_mul(std::mem::size_of::<scoop_identity::PersistentExactTypeId>() as u64), &path)?;
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
