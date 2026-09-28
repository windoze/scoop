use super::*;

pub(super) fn project<'a>(
    slots: &[mir::MirDispatchEntryV1],
    physical: &[lir::DispatchEntry],
    input: LayoutAbiExportInputV1<'a>,
    layouts: &'a lookup::Layouts<'_>,
    callables: &'a lir::CanonicalExactCallableAbiExportsV1,
    dependencies: LayoutAbiExportDependenciesV1<'a>,
) -> Result<Vec<lir::ExactDispatchEntryInputV1<'a>>, Error> {
    let mut entries = reserve(slots.len())?;
    for slot in slots {
        let implementation = implementation(slot.implementation());
        let position = slot.position().get();
        let emitted = physical
            .get(position as usize)
            .ok_or(lir::ExactDispatchError::MissingPhysicalCallable(position))?;
        let provider = match emitted.callable {
            lir::CallableRef::Local(_) => input.lir.foundation().producer(),
            lir::CallableRef::External(id) => {
                input.lir.module().meta.external_callables[id].provider()
            }
            lir::CallableRef::Runtime(_) => {
                return Err(Error::MissingCallable(implementation.target()));
            }
        };
        let abi = lookup::callable(
            implementation.target(),
            provider,
            callables,
            dependencies.callables,
            input.ordinary,
            dependencies.direct_callables,
            layouts,
        )?;
        let signature = slot.signature();
        let receiver = match implementation.receiver_adaptation() {
            lir::ExactDispatchReceiverAdaptationV1::Identity => None,
            lir::ExactDispatchReceiverAdaptationV1::ReferenceDispatch => signature
                .exact()
                .receiver()
                .into_option()
                .map(|exact| layouts.value(exact))
                .transpose()?,
        };

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
