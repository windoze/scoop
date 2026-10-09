use std::collections::HashSet;

use scoop_identity::{DispatchRole, DispatchSlotKey, GeneratedCallableKey};

use super::*;

impl MirDispatchSchemaAuthority<'_> {
    pub(super) fn validate_record(
        &self,
        record: &ParamFreeMirDispatchSchemaV1,
    ) -> Result<(), MirDispatchSchemaError> {
        let representation = self.type_export(record.owner())?.representation();
        let class_like = matches!(
            representation,
            MirTypeRepresentationV1::Intrinsic(
                crate::MirParamFreeIntrinsicV1::String
                    | crate::MirParamFreeIntrinsicV1::AtomicInt
                    | crate::MirParamFreeIntrinsicV1::AtomicLong
                    | crate::MirParamFreeIntrinsicV1::AtomicBoolean
                    | crate::MirParamFreeIntrinsicV1::Any
                    | crate::MirParamFreeIntrinsicV1::Nothing
            ) | MirTypeRepresentationV1::Class { .. }
                | MirTypeRepresentationV1::InlineArray { .. }
                | MirTypeRepresentationV1::AtomicReference { .. }
                | MirTypeRepresentationV1::Object { .. }
                | MirTypeRepresentationV1::ObjectBacking { .. }
        );
        let interface_owner = matches!(representation, MirTypeRepresentationV1::Interface);
        if class_like != matches!(record.slots(), MirDispatchSlotsV1::ClassVtable(_))
            || interface_owner != record.interface_slots().is_some()
            || (interface_owner && !record.itables().is_empty())
            || matches!(
                representation,
                MirTypeRepresentationV1::CoroutineStep { .. }
                    | MirTypeRepresentationV1::CoroutineSlot { .. }
            )
        {
            return Err(MirDispatchSchemaError::OwnerKind {
                owner: record.owner(),
            });
        }
        self.entries(record.owner(), None, record.vtable())?;
        if let Some(slots) = record.interface_slots() {
            self.slots(record.owner(), Some(record.owner()), slots.iter())?;
        }
        for (index, itable) in record.itables().iter().enumerate() {
            if index > 0 && record.itables()[index - 1].interface() >= itable.interface() {
                return Err(MirDispatchSchemaError::NonCanonicalInterfaceOrder { index });
            }
            if !matches!(
                self.type_export(itable.interface())?.representation(),
                MirTypeRepresentationV1::Interface
            ) {
                return Err(MirDispatchSchemaError::OwnerKind {
                    owner: itable.interface(),
                });
            }
            self.canonical_receiver_path(record.owner(), itable.interface())?;
            self.entries(record.owner(), Some(itable.interface()), itable.entries())?;
        }
        Ok(())
    }

    fn entries(
        &self,
        owner: PersistentExactTypeId,
        interface: Option<PersistentExactTypeId>,
        entries: &[MirDispatchEntryV1],
    ) -> Result<(), MirDispatchSchemaError> {
        self.slots(
            owner,
            interface,
            entries.iter().map(MirDispatchEntryV1::contract),
        )?;
        for entry in entries {
            self.implementation(owner, interface, entry)?;
        }
        Ok(())
    }

    fn slots<'a>(
        &self,
        owner: PersistentExactTypeId,
        interface: Option<PersistentExactTypeId>,
        slots: impl ExactSizeIterator<Item = &'a MirDispatchSlotV1>,
    ) -> Result<(), MirDispatchSchemaError> {
        let mut seen = HashSet::new();

        scoop_wire::allocation::try_reserve_set(&mut seen, slots.len(), &WirePath::root())?;
        for (index, entry) in slots.enumerate() {
            if usize::try_from(entry.position().get()).ok() != Some(index) {
                return Err(MirDispatchSchemaError::Position { index });
            }
            if !seen.insert(entry.slot()) {
                return Err(MirDispatchSchemaError::DuplicateSlot { slot: entry.slot() });
            }
            let key = self
                .identities
                .canonical_key::<_, DispatchSlotKey>(entry.slot())?;
            if matches!(
                (interface, key.role()),
                (Some(_), DispatchRole::VirtualMethod) | (None, DispatchRole::InterfaceMethod)
            ) {
                return Err(MirDispatchSchemaError::SlotRole { slot: entry.slot() });
            }
            let receiver = dispatch_declaration_receiver(
                self.identities,
                self.types,
                key.owner(),
                interface.unwrap_or(owner),
            )
            .map_err(|error| MirDispatchSchemaError::Signature(Box::new(error)))?;
            let receiver_is_interface = matches!(
                self.type_export(receiver)?.representation(),
                MirTypeRepresentationV1::Interface
            );
            if receiver_is_interface != interface.is_some() {
                return Err(MirDispatchSchemaError::SlotRole { slot: entry.slot() });
            }
            self.canonical_receiver_path(interface.unwrap_or(owner), receiver)?;
            if entry.signature().exact().receiver().into_option()
                != Some(interface.unwrap_or(receiver))
                || entry.signature().gc_effect() != crate::GcEffect::Managed
            {
                return Err(MirDispatchSchemaError::SlotSignature { slot: entry.slot() });
            }
        }
        Ok(())
    }

    fn callable(
        &self,
        target: CallableDefinitionOwner,
    ) -> Result<MirCallableRecordRefV1<'_>, MirDispatchSchemaError> {
        self.callables
            .get(target)
            .ok_or(MirDispatchSchemaError::MissingCallable { target })
    }

    fn implementation(
        &self,
        owner: PersistentExactTypeId,
        interface: Option<PersistentExactTypeId>,
        entry: &MirDispatchEntryV1,
    ) -> Result<(), MirDispatchSchemaError> {
        let target = self.callable(entry.implementation().target())?;
        let invalid = || MirDispatchSchemaError::InvalidImplementation { slot: entry.slot() };
        match entry.implementation() {
            MirDispatchImplementationV1::AbstractObligation {
                declaration: supplied,
                trap_target,
                receiver,
            } => {
                if dispatch_declaration_target(
                    self.identities,
                    self.types,
                    supplied,
                    target
                        .lowered_signature()
                        .exact()
                        .receiver()
                        .into_option()
                        .ok_or_else(invalid)?,
                )
                .map_err(|error| MirDispatchSchemaError::Signature(Box::new(error)))?
                    != trap_target
                    || !matches!(
                        target.lowering_role(),
                        MirCallableLoweringRoleV1::PureVirtualTrap { .. }
                    )
                {
                    return Err(invalid());
                }
                self.direct_receiver(owner, entry, target.lowered_signature(), receiver)?;
                if !matches!(
                    self.type_export(owner)?.representation(),
                    MirTypeRepresentationV1::Interface
                        | MirTypeRepresentationV1::Class {
                            kind: MirClassKindV1::Abstract,
                            ..
                        }
                ) {
                    return Err(MirDispatchSchemaError::ConcreteObligation {
                        owner,
                        slot: entry.slot(),
                    });
                }
            }
            MirDispatchImplementationV1::DirectStrongTarget { receiver, .. }
            | MirDispatchImplementationV1::InterfaceDefaultTarget { receiver, .. } => {
                if self.type_export(owner)?.facts().kind() != MirValueKindV1::Reference
                    || !matches!(
                        target.lowering_role(),
                        MirCallableLoweringRoleV1::Ordinary
                            | MirCallableLoweringRoleV1::Accessor
                            | MirCallableLoweringRoleV1::DerivedEquality { .. }
                    )
                {
                    return Err(invalid());
                }
                if matches!(
                    entry.implementation(),
                    MirDispatchImplementationV1::InterfaceDefaultTarget { .. }
                ) {
                    let receiver = target
                        .lowered_signature()
                        .exact()
                        .receiver()
                        .into_option()
                        .ok_or_else(invalid)?;
                    if !matches!(
                        self.type_export(receiver)?.representation(),
                        MirTypeRepresentationV1::Interface
                    ) {
                        return Err(invalid());
                    }
                }
                self.direct_receiver(owner, entry, target.lowered_signature(), receiver)?;
            }
            MirDispatchImplementationV1::AdjustThunkTarget(_) => {
                let origin = target.origin();
                let (semantic_target, generated) = match (target.lowering_role(), origin.as_ref()) {
                    (
                        MirCallableLoweringRoleV1::DispatchAdjust { target }
                        | MirCallableLoweringRoleV1::BoxingAdjust { target },
                        MirCallableOriginV1::Generated { role, .. },
                    ) => (*target, role),
                    _ => return Err(invalid()),
                };
                let relation = match generated {
                    GeneratedCallableKey::DispatchAdjust {
                        slot, implementor, ..
                    } => {
                        *slot == entry.slot()
                            && (*implementor == owner
                                || matches!(self.type_export(*implementor)?.representation(),
                                    MirTypeRepresentationV1::Object { backing } if *backing == owner))
                    }
                    GeneratedCallableKey::BoxingAdjust {
                        slot,
                        payload,
                        interface: expected,
                    } => {
                        let payload_owner = match self.type_export(owner)?.representation() {
                            MirTypeRepresentationV1::BoxedValue { payload } => payload.value,
                            _ => owner,
                        };
                        *slot == entry.slot()
                            && *payload == payload_owner
                            && interface == Some(*expected)
                    }
                    _ => false,
                };
                if let Some(receiver) = target.semantic_signature().exact().receiver().into_option()
                {
                    self.canonical_receiver_path(owner, receiver)?;
                }
                let receiver = match generated {
                    GeneratedCallableKey::DispatchAdjust { implementor, .. } => *implementor,
                    GeneratedCallableKey::BoxingAdjust { payload, .. } => {
                        crate::InterfaceAdjustIdentity::boxed_receiver(*payload)
                            .map_err(|_| invalid())?
                    }
                    _ => return Err(invalid()),
                };
                if !relation
                    || !same_non_receiver(target.lowered_signature(), entry.signature())
                    || target.lowered_signature().exact().receiver().into_option() != Some(receiver)
                    || self.callable(semantic_target)?.semantic_signature()
                        != target.semantic_signature()
                {
                    return Err(invalid());
                }
            }
        }
        Ok(())
    }

    fn direct_receiver(
        &self,
        owner: PersistentExactTypeId,
        entry: &MirDispatchEntryV1,
        target: &MirBridgeCallableSignatureV1,
        adaptation: MirDispatchReceiverAdaptationV1,
    ) -> Result<(), MirDispatchSchemaError> {
        let mismatch = || MirDispatchSchemaError::TargetSignature { slot: entry.slot() };
        if adaptation == MirDispatchReceiverAdaptationV1::Identity {
            return if entry.signature() == target {
                Ok(())
            } else {
                Err(mismatch())
            };
        }
        if !same_non_receiver(entry.signature(), target)
            || entry.signature().exact().receiver() == target.exact().receiver()
        {
            return Err(mismatch());
        }
        for receiver in [
            entry.signature().exact().receiver(),
            target.exact().receiver(),
        ] {
            let receiver = receiver.into_option().ok_or_else(mismatch)?;
            let path = self.canonical_receiver_path(owner, receiver)?;
            for exact in path {
                if self.type_export(exact)?.facts().kind() != MirValueKindV1::Reference {
                    return Err(mismatch());
                }
            }
        }
        Ok(())
    }
}

pub(super) fn same_non_receiver(
    left: &MirBridgeCallableSignatureV1,
    right: &MirBridgeCallableSignatureV1,
) -> bool {
    left.gc_effect() == right.gc_effect()
        && left.exact().effect() == right.exact().effect()
        && left.exact().parameters() == right.exact().parameters()
        && left.exact().result() == right.exact().result()
}
