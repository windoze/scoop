use super::*;

impl<'a> Graph<'a> {
    fn provider_for(
        &self,
        owner: PersistentTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<&Provider<'a>, Error> {
        let path = WirePath::root().field(8);
        meter.charge_work(
            1 + u64::from(self.current.identities.identity_count().max(1).ilog2()),
            &path,
        )?;
        let source = self
            .current
            .identities
            .canonical_key::<_, SourceDeclarationKey>(owner)?;
        let provider = source.origin();
        meter.charge_work(1 + u64::from(self.providers.len().max(1).ilog2()), &path)?;
        self.providers
            .get(&provider)
            .ok_or(Error::MissingProvider(provider))
    }

    pub(super) fn nominal(
        &self,
        owner: PersistentTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<&'a crate::NominalInterfaceRecordV1, Error> {
        let terminal = self.provider_for(owner, meter)?;
        let declarations = terminal.metadata.public.nominal_interfaces();
        meter.charge_work(
            1 + u64::from(declarations.declaration_count().max(1).ilog2()),
            &WirePath::root().field(8),
        )?;
        declarations
            .declaration(SourceNominalId::Concrete(owner))
            .ok_or(Error::MissingNominal(owner))
    }

    pub(super) fn resolve_nominal(
        &self,
        owner: PersistentTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<(ConeIdentity, PersistentExactTypeId), Error> {
        let path = WirePath::root().field(8);
        let terminal = self.provider_for(owner, meter)?;
        let builtin = [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any]
            .iter()
            .any(|builtin| builtin.identity_record().id() == owner);
        if !builtin {
            self.nominal(owner, meter)?;
            meter.charge_work(
                1 + u64::from(terminal.materialization.sources().len().max(1).ilog2()),
                &path,
            )?;
            if !terminal.materialization.contains(owner) {
                return Err(Error::SourceOnlyNominal(owner));
            }
        }
        let key = ExactTypeKey::Nominal(owner);
        meter.charge_sha256(
            PersistentExactTypeId::hash_stream_length(&key)
                .map_err(|e| Error::Key(e.to_string()))?,
            &path,
        )?;
        let exact = PersistentExactTypeId::from_key(&key).map_err(|e| Error::Key(e.to_string()))?;
        meter.charge_work(
            1 + u64::from(terminal.metadata.identities.identity_count().max(1).ilog2()),
            &path,
        )?;
        if terminal
            .metadata
            .identities
            .canonical_key::<_, ExactTypeKey>(exact)?
            .as_ref()
            != &key
        {
            return Err(Error::NominalOwner(owner));
        }
        Ok((terminal.metadata.provider, exact))
    }
}
