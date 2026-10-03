use super::*;
use scoop_identity::ExactCallableSignature;

pub(super) fn signature(
    source: &hir::InheritanceCallableSignatureV1,
    receiver: Option<PersistentExactTypeId>,
) -> Result<mir::MirBridgeCallableSignatureV1, Error> {
    Ok(mir::MirBridgeCallableSignatureV1::new(
        exact_signature(source, receiver)?,
        match source.effects().gc_effect() {
            scoop_identity::GcEffect::Managed => mir::GcEffect::Managed,
            scoop_identity::GcEffect::NoGc => mir::GcEffect::NoGc,
        },
    ))
}

fn exact_signature(
    source: &hir::InheritanceCallableSignatureV1,
    receiver: Option<PersistentExactTypeId>,
) -> Result<ExactCallableSignature, Error> {
    let exact = source.exact_signature();
    let mut parameters = Vec::new();
    scoop_wire::allocation::try_reserve(
        &mut parameters,
        exact.parameters().len(),
        &WirePath::root(),
    )?;

    parameters.extend_from_slice(exact.parameters());
    Ok(ExactCallableSignature::new(
        exact.effect(),
        receiver,
        parameters,
        exact.result(),
    ))
}

impl Replay<'_> {
    pub(super) fn binding(
        &self,
        target: CallableDefinitionOwner,
    ) -> Result<mir::MirCallableRecordRefV1<'_>, Error> {
        self.callables
            .get(target)
            .ok_or(Error::MissingCallable(target))
    }

    pub(super) fn source_binding(
        &self,
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
        target: CallableDefinitionOwner,
        signature: &mir::MirBridgeCallableSignatureV1,
    ) -> Result<(), Error> {
        let binding = self.binding(target)?;

        Error::entry(
            owner,
            slot,
            Component::CallableSignature,
            binding.semantic_signature() == signature,
        )
    }

    pub(super) fn adjustment(
        &mut self,
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
        interface: PersistentExactTypeId,
        source: &hir::InheritanceSlotTargetV1,
    ) -> Result<CallableDefinitionOwner, Error> {
        let key = GeneratedCallableKey::BoxingAdjust {
            slot,
            payload: owner,
            interface,
        };

        let callable = PersistentGeneratedCallableId::from_key(&key).map_err(Error::Key)?;
        let target = self.target(source)?;
        let semantic = signature(
            source.signature(),
            source
                .signature()
                .exact_signature()
                .receiver()
                .into_option(),
        )?;
        self.source_binding(owner, slot, target, &semantic)?;
        let lowered = self.lowered_signature(&mir::MirBridgeCallableSignatureV1::new(
            exact_signature(source.signature(), Some(interface))?,
            mir::GcEffect::Managed,
        ))?;
        let implementation = StrongCallableDefinitionOwner::GeneratedCallable(callable).into();
        let binding = self.binding(implementation)?;

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
            binding.origin().as_ref()
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
        insert(&mut self.adjustments, callable)?;
        Ok(implementation)
    }
}
