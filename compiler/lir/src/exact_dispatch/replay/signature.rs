use super::*;
use crate::{ExactRepresentationKindV1, NichePointerKind};
use scoop_identity::RepresentationRole;

pub(super) fn validate(
    input: &ExactDispatchEntryInputV1<'_>,
    target: crate::LirTargetProfile,
) -> Result<Option<std::sync::Arc<crate::ExactValueLayoutV1>>, ExactDispatchError> {
    let owner = input.implementation.target();
    let target_signature = input.abi.canonical_signature();
    if input.slot_signature.gc_effect() != target_signature.gc_effect() {
        return Err(ExactDispatchError::AbiSignature(owner));
    }
    let slot = input.slot_signature.exact();
    let implementation = target_signature.signature();
    match input.implementation.receiver_adaptation() {
        ExactDispatchReceiverAdaptationV1::Identity => {
            if slot != implementation {
                return Err(ExactDispatchError::AbiSignature(owner));
            }
            if input.slot_receiver_layout.is_some() {
                return Err(ExactDispatchError::UnexpectedReceiverLayout(owner));
            }
            Ok(None)
        }
        ExactDispatchReceiverAdaptationV1::ReferenceDispatch => {
            if slot.effect() != implementation.effect()
                || slot.parameters() != implementation.parameters()
                || slot.result() != implementation.result()
            {
                return Err(ExactDispatchError::AbiSignature(owner));
            }
            let (Some(slot_receiver), Some(implementation_receiver)) = (
                slot.receiver().into_option(),
                implementation.receiver().into_option(),
            ) else {
                return Err(ExactDispatchError::ReceiverAdaptation(owner));
            };
            if slot_receiver == implementation_receiver {
                return Err(ExactDispatchError::ReceiverAdaptation(owner));
            }
            let Some(slot_layout) = input.slot_receiver_layout else {
                return Err(ExactDispatchError::ReceiverLayout(owner));
            };
            if slot_layout.identity().target() != target
                || slot_layout.identity().exact() != slot_receiver
                || slot_layout.identity().layout_key().representation()
                    != RepresentationRole::ManagedValue
            {
                return Err(ExactDispatchError::ReceiverLayout(owner));
            }
            let Some(slot_layout) = slot_layout.value_handle() else {
                return Err(ExactDispatchError::ReceiverLayout(owner));
            };
            let Some(target_layout) = input.abi.receiver_layout()? else {
                return Err(ExactDispatchError::ReceiverLayout(owner));
            };
            if !managed_reference(&slot_layout) || !managed_reference(&target_layout) {
                return Err(ExactDispatchError::ReceiverLayout(owner));
            }
            Ok(Some(slot_layout))
        }
    }
}

fn managed_reference(value: &crate::ExactValueLayoutV1) -> bool {
    matches!(
        value.representation().kind(),
        ExactRepresentationKindV1::QualifiedPointer(NichePointerKind::Managed)
    )
}
