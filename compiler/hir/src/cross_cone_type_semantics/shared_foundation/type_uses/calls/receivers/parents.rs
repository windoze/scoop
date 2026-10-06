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
        if matches!(key.as_ref(), ExactTypeKey::Tuple(_)) {
            let dependencies = self
                .providers
                .values()
                .map(|provider| provider.metadata)
                .collect::<Vec<_>>();
            return self
                .current
                .tuple_encoding_parent(receiver, &dependencies)
                .map(|parent| parent.into_iter().collect());
        }
        // Any has no source declaration; Unit members use its ordinary core declaration.
        if let ExactTypeKey::Nominal(owner) = key.as_ref() {
            self.resolve_nominal(*owner)?;
            if CoreBuiltinNominal::Any.identity_record().id() == *owner {
                return Ok(Vec::new());
            }
        }
        let dependencies = self.providers.values().map(|provider| provider.metadata);
        let application = self.current.applied_nominal(receiver, dependencies)?;
        let bindings = application.bindings();
        let supertypes = application.declaration.exact_supertypes().values();
        let mut parents = Vec::new();
        scoop_wire::allocation::try_reserve(&mut parents, supertypes.len(), path)?;
        for parent in supertypes {
            let exact = self.current.signature_exact_type_with_bindings(
                parent,
                &bindings,
                self.current.identities,
            )?;
            let parent = self.current.applied_nominal(
                exact,
                self.providers.values().map(|provider| provider.metadata),
            )?;
            if !matches!(
                parent.declaration.kind(),
                crate::PublicNominalKindV1::Class | crate::PublicNominalKindV1::Interface
            ) {
                return Err(Error::InheritanceEdges(receiver));
            }
            parents.push(exact);
        }
        let dependencies = self
            .providers
            .values()
            .map(|provider| provider.metadata)
            .collect::<Vec<_>>();
        if let Some(parent) = self
            .current
            .applied_encoding_parent(&application, &dependencies)?
        {
            parents.push(parent);
        }
        Ok(parents)
    }
}
