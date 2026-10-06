use super::*;

impl<'a> SharedTypeMetadataV1<'a> {
    pub fn tuple_encoding_parent(
        self,
        owner: PersistentExactTypeId,
        dependencies: &[Self],
    ) -> Result<Option<PersistentExactTypeId>, Error> {
        if !matches!(
            self.identities
                .canonical_key::<_, ExactTypeKey>(owner)?
                .as_ref(),
            ExactTypeKey::Tuple(_)
        ) {
            return Ok(None);
        }
        let Some(core) = std::iter::once(self)
            .chain(dependencies.iter().copied())
            .find(|metadata| metadata.provider == scoop_identity::ConeIdentity::CORE)
        else {
            return Ok(None);
        };
        let Some(encoding) = core
            .public
            .nominal_interfaces()
            .all_records()
            .find_map(|declaration| declaration.declaration_details().element_encoding())
        else {
            return Ok(None);
        };
        let SignatureTypeKey::Nominal(interface) = encoding.interface() else {
            return Err(Error::NonConcreteSignature);
        };
        if self.element_has_encoding(owner, SourceNominalId::Concrete(*interface), dependencies)? {
            self.signature_exact_type(encoding.interface()).map(Some)
        } else {
            Ok(None)
        }
    }
}
