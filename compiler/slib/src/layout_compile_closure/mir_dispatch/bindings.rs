use super::*;
use scoop_identity::ExactCallableSignature;

pub(super) fn signature(
    source: &hir::InheritanceCallableSignatureV1,
    receiver: Option<PersistentExactTypeId>,
    meter: &mut BudgetMeter,
) -> Result<mir::MirBridgeCallableSignatureV1, Error> {
    Ok(mir::MirBridgeCallableSignatureV1::new(
        exact_signature(source, receiver, meter)?,
        match source.effects().gc_effect() {
            scoop_identity::GcEffect::Managed => mir::GcEffect::Managed,
            scoop_identity::GcEffect::NoGc => mir::GcEffect::NoGc,
        },
    ))
}

fn exact_signature(
    source: &hir::InheritanceCallableSignatureV1,
    receiver: Option<PersistentExactTypeId>,
    meter: &mut BudgetMeter,
) -> Result<ExactCallableSignature, Error> {
    let exact = source.exact_signature();
    let mut parameters = Vec::new();
    meter.try_reserve_collection_slots(
        &mut parameters,
        exact.parameters().len(),
        &WirePath::root(),
    )?;
    meter.charge_work(exact.parameters().len() as u64 + 1, &WirePath::root())?;
    parameters.extend_from_slice(exact.parameters());
    Ok(ExactCallableSignature::new(
        exact.effect(),
        receiver,
        parameters,
        exact.result(),
    ))
}

impl Replay<'_, '_> {
    pub(super) fn binding(
        &self,
        target: StrongCallableDefinitionOwner,
        meter: &mut BudgetMeter,
    ) -> Result<&mir::ParamFreeMirCallableBindingV1, Error> {
        lookup(self.callables.record_count(), meter)?;
        self.callables
            .get(target)
            .ok_or(Error::MissingCallable(target))
    }

    pub(super) fn source_binding(
        &self,
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
        target: StrongCallableDefinitionOwner,
        signature: &mir::MirBridgeCallableSignatureV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        let binding = self.binding(target, meter)?;
        meter.charge_work(
            signature.exact().parameters().len() as u64 * 2 + 10,
            &WirePath::root(),
        )?;
        Error::entry(
            owner,
            slot,
            Component::CallableSignature,
            binding.semantic_signature() == signature && binding.lowered_signature() == signature,
        )
    }

    pub(super) fn adjustment(
        &mut self,
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
        interface: PersistentExactTypeId,
        source: &hir::InheritanceSlotTargetV1,
        meter: &mut BudgetMeter,
    ) -> Result<StrongCallableDefinitionOwner, Error> {
        let key = GeneratedCallableKey::BoxingAdjust {
            slot,
            payload: owner,
            interface,
        };
        meter.charge_sha256(
            PersistentGeneratedCallableId::hash_stream_length(&key).map_err(Error::Key)?,
            &WirePath::root(),
        )?;
        let callable = PersistentGeneratedCallableId::from_key(&key).map_err(Error::Key)?;
        let target = target(source.declaration());
        let semantic = signature(
            source.signature(),
            source
                .signature()
                .exact_signature()
                .receiver()
                .into_option(),
            meter,
        )?;
        self.source_binding(owner, slot, target, &semantic, meter)?;
        let lowered = mir::MirBridgeCallableSignatureV1::new(
            exact_signature(source.signature(), Some(interface), meter)?,
            mir::GcEffect::Managed,
        );
        let implementation = StrongCallableDefinitionOwner::GeneratedCallable(callable);
        let binding = self.binding(implementation, meter)?;
        meter.charge_work(
            (semantic.exact().parameters().len() + lowered.exact().parameters().len()) as u64 + 12,
            &WirePath::root(),
        )?;
        Error::entry(
            owner,
            slot,
            Component::CallableSignature,
            binding.semantic_signature() == &semantic && binding.lowered_signature() == &lowered,
        )?;
        Error::entry(
            owner,
            slot,
            Component::CallableOrigin,
            binding.origin()
                == &mir::MirCallableOriginV1::Generated {
                    callable,
                    role: key,
                },
        )?;
        Error::entry(
            owner,
            slot,
            Component::CallableRole,
            binding.lowering_role() == &mir::MirCallableLoweringRoleV1::BoxingAdjust { target },
        )?;
        insert(&mut self.adjustments, callable, meter)?;
        Ok(implementation)
    }
}
