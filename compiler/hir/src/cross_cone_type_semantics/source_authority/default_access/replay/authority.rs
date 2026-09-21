use super::*;
use scoop_identity::DefinitionOriginSubject;

fn query(count: usize, meter: &mut BudgetMeter, path: &WirePath) -> Result<(), WireError> {
    meter.charge_work((u64::from(count.max(1).ilog2()) + 1) * 65, path)
}
impl SourceDomainAuthority for BoundTypeFoundationSourcesV1<'_> {
    type Error = TypeFoundationBindingError;
    fn nominal_key(
        &self,
        owner: SourceNominalId,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<&SourceDeclarationKey, Self::Error> {
        query(self.source().entries().sources.records().len(), meter, path)?;
        BoundTypeFoundationSourcesV1::nominal_key(self, owner)
    }
    fn nominal_access(
        &self,
        owner: SourceNominalId,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<&DeclarationAccessSourceV1, Self::Error> {
        query(self.source().entries().sources.records().len(), meter, path)?;
        self.nominal_source(owner).map(TypeSourceNominalV1::access)
    }
    fn exact_key(
        &self,
        exact: PersistentExactTypeId,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<&ExactTypeKey, Self::Error> {
        query(
            self.source().entries().exact_keys.values().len(),
            meter,
            path,
        )?;
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
    fn nominal_key(
        &self,
        owner: SourceNominalId,
        meter: &mut BudgetMeter,
        _: &WirePath,
    ) -> Result<&SourceDeclarationKey, Self::Error> {
        self.source_key(subject(owner), meter)
    }
    fn nominal_access(
        &self,
        owner: SourceNominalId,
        meter: &mut BudgetMeter,
        _: &WirePath,
    ) -> Result<&DeclarationAccessSourceV1, Self::Error> {
        self.declaration(subject(owner), meter)
            .map(DefaultSourceAccessDeclarationV1::declaration_access)
    }
    fn exact_key(
        &self,
        exact: PersistentExactTypeId,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<&ExactTypeKey, Self::Error> {
        SourceDomainAuthority::exact_key(self.foundation, exact, meter, path).map_err(Into::into)
    }
}
impl BoundDefaultSourceAccessDeclarationsV1<'_, '_, '_> {
    /// Replays the complete lexical visibility conjunction. This does not
    /// normalize inheritance, check receivers, or grant callable capability.
    pub fn source_lookup_domain(
        &self,
        subject: DefinitionOriginSubject,
        meter: &mut BudgetMeter,
    ) -> Result<DefaultSourceAccessDomainV1, Error<DefaultSourceAccessBindingError>> {
        let path = WirePath::root();
        meter.check_semantic_depth(1, &path)?;
        let access = self
            .declaration(subject, meter)
            .map_err(Error::Authority)?
            .declaration_access();
        lookup_domain(access, self, meter, &path)
    }
}
