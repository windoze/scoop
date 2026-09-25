use super::*;
use scoop_identity::DefinitionOriginSubject;

impl SourceDomainAuthority for BoundTypeFoundationSourcesV1<'_> {
    type Error = TypeFoundationBindingError;
    fn nominal_key(&self, owner: SourceNominalId) -> Result<&SourceDeclarationKey, Self::Error> {
        BoundTypeFoundationSourcesV1::nominal_key(self, owner)
    }
    fn nominal_access(
        &self,
        owner: SourceNominalId,
    ) -> Result<&DeclarationAccessSourceV1, Self::Error> {
        self.nominal_source(owner).map(TypeSourceNominalV1::access)
    }
    fn exact_key(&self, exact: PersistentExactTypeId) -> Result<&ExactTypeKey, Self::Error> {
        self.exact_type_key(exact)
    }
}
fn subject(owner: SourceNominalId) -> DefinitionOriginSubject {
    match owner {
        SourceNominalId::Concrete(id) => DefinitionOriginSubject::Type(id),
        SourceNominalId::GenericTemplate(id) => DefinitionOriginSubject::GenericType(id),
    }
}
impl SourceDomainAuthority for BoundDefaultSourceAccessDeclarationsV1<'_, '_, '_> {
    type Error = DefaultSourceAccessBindingError;
    fn nominal_key(&self, owner: SourceNominalId) -> Result<&SourceDeclarationKey, Self::Error> {
        self.source_key(subject(owner))
    }
    fn nominal_access(
        &self,
        owner: SourceNominalId,
    ) -> Result<&DeclarationAccessSourceV1, Self::Error> {
        self.declaration(subject(owner))
            .map(DefaultSourceAccessDeclarationV1::declaration_access)
    }
    fn exact_key(&self, exact: PersistentExactTypeId) -> Result<&ExactTypeKey, Self::Error> {
        SourceDomainAuthority::exact_key(self.foundation, exact).map_err(Into::into)
    }
}
impl BoundDefaultSourceAccessDeclarationsV1<'_, '_, '_> {
    /// Replays the complete lexical visibility conjunction. This does not
    /// normalize inheritance, check receivers, or grant callable capability.
    pub fn source_lookup_domain(
        &self,
        subject: DefinitionOriginSubject,
    ) -> Result<DefaultSourceAccessDomainV1, Error<DefaultSourceAccessBindingError>> {
        let path = WirePath::root();

        let access = self
            .declaration(subject)
            .map_err(Error::Authority)?
            .declaration_access();
        lookup_domain(access, self, &path)
    }
}
