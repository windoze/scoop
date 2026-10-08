use super::*;

impl SlotContracts<'_> {
    pub(super) fn selected_target(
        &self,
        receiver: PersistentExactTypeId,
        declaration: Declaration,
        slot: &InheritanceSlotTargetV1,
    ) -> Result<InheritanceSlotTargetV1, Error> {
        let Declaration::DerivedEquality(owner) = declaration else {
            return self.target(receiver, declaration);
        };
        let exact = slot.signature().exact_signature();
        if exact.parameters() != [receiver] || !slot.signature().context_keys().is_empty() {
            return Err(invalid(
                "derived equality must implement a same-owner comparison slot",
            ));
        }
        let metadata = std::iter::once(self.metadata)
            .chain(self.dependencies.iter().copied())
            .find(|metadata| {
                metadata
                    .public
                    .nominal_interfaces()
                    .declaration(owner)
                    .is_some()
            })
            .ok_or_else(|| invalid("derived equality has no nominal declaration"))?;
        let expected = self
            .metadata
            .applied_member_receiver(receiver, owner, &self.dependencies)
            .map_err(invalid)?;
        if expected != receiver {
            return Err(invalid(
                "derived equality has a different receiver application",
            ));
        }
        let effects = slot.signature().effects();
        let effects = CallableSourceEffectsV1::try_new(
            effects.execution(),
            effects.safety(),
            effects.gc_effect(),
            CallableImplementationV1::Scoop,
            effects.operator_role(),
            effects.infix(),
        )
        .map_err(invalid)?;
        let signature = InheritanceCallableSignatureV1::try_new(
            scoop_identity::ExactCallableSignature::new(
                exact.effect(),
                Some(receiver),
                vec![receiver],
                exact.result(),
            ),
            effects,
            Vec::new(),
        )
        .map_err(invalid)?;
        Ok(InheritanceSlotTargetV1::new(
            declaration,
            signature,
            CallableModalityV1::Final,
            metadata.derived_equality_access(owner).map_err(invalid)?,
        ))
    }
}
