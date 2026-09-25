use super::*;

impl DefaultSourceDomainsV1<'_, '_, '_, '_> {
    pub(super) fn equality_domain(
        &self,
        owner: &SignatureTypeKey,
        scope: &SignatureBinderScopeV1,

        path: &WirePath,
    ) -> Result<DefaultSourceAccessDomainV1, Error> {
        let (nominal, arity) = match owner {
            SignatureTypeKey::Nominal(id) if *id == self.core.unit().persistent() => {
                return self.type_source_domain_at(owner, scope, path);
            }
            SignatureTypeKey::Tuple(_) => {
                return self.type_source_domain_at(owner, scope, path);
            }
            SignatureTypeKey::Nominal(id) => (SourceNominalId::Concrete(*id), 0),
            SignatureTypeKey::NominalApplication { origin, arguments } => (
                SourceNominalId::GenericTemplate(*origin),
                arguments.as_slice().len(),
            ),
            _ => return Err(Error::EqualityShape),
        };
        let provider = self.provider(self.nominal_provider(nominal)?)?;
        let key = provider
            .source_key(subject(nominal))
            .map_err(Error::access)?;
        if !matches!(
            key.declaration_kind(),
            SourceDeclarationKind::Struct | SourceDeclarationKind::Enum
        ) {
            return Err(Error::EqualityKind(nominal));
        }
        // The nominal helper uses the declaration's domain. Its applied type
        // arguments remain independent type-access demands in the same body.
        self.nominal_domain(nominal, arity)
    }
}
