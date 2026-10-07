use super::*;

impl<'a> Graph<'a> {
    fn provider_for(&self, owner: PersistentTypeId) -> Result<&Provider<'a>, Error> {
        let source = self
            .current
            .identities
            .canonical_key::<_, SourceDeclarationKey>(owner)?;
        let provider = source.origin();

        self.providers
            .get(&provider)
            .ok_or(Error::MissingProvider(provider))
    }

    pub(super) fn nominal(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&'a crate::NominalInterfaceRecordV1, Error> {
        let terminal = self.provider_for(owner)?;
        let declarations = terminal.metadata.public.nominal_interfaces();

        declarations
            .declaration(SourceNominalId::Concrete(owner))
            .ok_or(Error::MissingNominal(owner))
    }

    pub(super) fn resolve_nominal(
        &self,
        owner: PersistentTypeId,
    ) -> Result<(ConeIdentity, PersistentExactTypeId), Error> {
        let terminal = self.provider_for(owner)?;
        let builtin = CoreBuiltinNominal::Unit.identity_record().id() == owner;
        if !builtin {
            self.nominal(owner)?;

            if !terminal.materialization.contains(owner) {
                return Err(Error::SourceOnlyNominal(owner));
            }
        }
        let key = ExactTypeKey::Nominal(owner);

        let exact = PersistentExactTypeId::from_key(&key).map_err(|e| Error::Key(e.to_string()))?;

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
