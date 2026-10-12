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
            return Ok(Vec::new());
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
            let integer_parent = matches!(application.declaration.source_shape(),
                crate::NominalSourceShapeV1::Enum(shape) if shape.representation().integer_kind().is_some());
            if !integer_parent
                && !matches!(
                    parent.declaration.kind(),
                    crate::PublicNominalKindV1::Class | crate::PublicNominalKindV1::Interface
                )
            {
                return Err(Error::InheritanceEdges(receiver));
            }
            parents.push(exact);
        }
        Ok(parents)
    }
}
