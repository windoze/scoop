use super::*;

impl Graph<'_> {
    pub(super) fn member_receiver_parents(
        &self,
        receiver: PersistentExactTypeId,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Vec<PersistentExactTypeId>, Error> {
        meter.charge_work(
            1 + u64::from(self.current.identities.identity_count().max(1).ilog2()),
            path,
        )?;
        let key = self
            .current
            .identities
            .canonical_key::<_, ExactTypeKey>(receiver)?;
        let ExactTypeKey::Nominal(owner) = key.as_ref() else {
            return Err(Error::NonConcreteSignature);
        };
        self.resolve_nominal(*owner, meter)?;
        let mut parents = Vec::new();
        // Language builtins have no ordinary source nominal declaration.
        if [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any]
            .iter()
            .any(|builtin| builtin.identity_record().id() == *owner)
        {
            return Ok(parents);
        }
        let source = self.nominal(*owner, meter)?;
        let supertypes = source.exact_supertypes().values();
        meter.try_reserve_collection_slots(&mut parents, supertypes.len(), path)?;
        for parent in supertypes {
            meter.charge_work(1, path)?;
            let SignatureTypeKey::Nominal(parent) = parent else {
                return Err(Error::NonConcreteSignature);
            };
            if !matches!(
                self.nominal(*parent, meter)?.kind(),
                crate::PublicNominalKindV1::Class | crate::PublicNominalKindV1::Interface
            ) {
                return Err(Error::InheritanceEdges(receiver));
            }
            parents.push(self.resolve_nominal(*parent, meter)?.1);
        }
        Ok(parents)
    }
}
