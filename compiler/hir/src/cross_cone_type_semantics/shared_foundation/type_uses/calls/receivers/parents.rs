use super::*;

impl Graph<'_> {
    pub(in super::super) fn source_receiver_parents(
        &self,
        receiver: PersistentExactTypeId,

        path: &WirePath,
    ) -> Result<Vec<PersistentExactTypeId>, Error> {
        let key = self
            .current
            .identities
            .canonical_key::<_, ExactTypeKey>(receiver)?;
        let ExactTypeKey::Nominal(owner) = key.as_ref() else {
            return Err(Error::NonConcreteSignature);
        };
        self.resolve_nominal(*owner)?;
        let mut parents = Vec::new();
        // Language builtins have no ordinary source nominal declaration.
        if [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any]
            .iter()
            .any(|builtin| builtin.identity_record().id() == *owner)
        {
            return Ok(parents);
        }
        let source = self.nominal(*owner)?;
        let supertypes = source.exact_supertypes().values();
        scoop_wire::allocation::try_reserve(&mut parents, supertypes.len(), path)?;
        for parent in supertypes {
            let SignatureTypeKey::Nominal(parent) = parent else {
                return Err(Error::NonConcreteSignature);
            };
            if !matches!(
                self.nominal(*parent)?.kind(),
                crate::PublicNominalKindV1::Class | crate::PublicNominalKindV1::Interface
            ) {
                return Err(Error::InheritanceEdges(receiver));
            }
            parents.push(self.resolve_nominal(*parent)?.1);
        }
        Ok(parents)
    }
}
