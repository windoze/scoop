use super::*;

impl NestedNominalSemanticAuthority<Error> for BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_> {
    fn nominal_source_binders(
        &self,
        owner: SourceNominalId,
    ) -> Result<&CanonicalBinderListV1, Error> {
        self.parameters
            .members()
            .nominals
            .nominal_source(owner)
            .map(NominalSourceContractV1::type_parameters)
            .map_err(Into::into)
    }
    fn nominal_source_supertypes(
        &self,
        owner: SourceNominalId,
    ) -> Result<&CanonicalSignatureTypesV1, Error> {
        self.parameters
            .members()
            .nominals
            .nominal_source(owner)
            .map(NominalSourceContractV1::supertypes)
            .map_err(Into::into)
    }
    fn nominal_source_constructors(
        &self,
        owner: SourceNominalId,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentConstructorId>, Error> {
        self.parameters
            .members()
            .nominals
            .nominal_source(owner)
            .map(NominalSourceContractV1::constructors)
            .map_err(Into::into)
    }
    fn nominal_source_members(
        &self,
        owner: SourceNominalId,
    ) -> Result<&CanonicalNestedMemberRefsV1, Error> {
        self.parameters
            .members()
            .nominals
            .nominal_source(owner)
            .map(NominalSourceContractV1::members)
            .map_err(Into::into)
    }
    fn nominal_source_children(
        &self,
        owner: SourceNominalId,
    ) -> Result<&CanonicalNestedNominalRefsV1, Error> {
        self.parameters
            .members()
            .nominals
            .nominal_source(owner)
            .map(NominalSourceContractV1::children)
            .map_err(Into::into)
    }
    fn nominal_source_shape(&self, owner: SourceNominalId) -> Result<&NominalSourceShapeV1, Error> {
        self.parameters
            .members()
            .nominals
            .nominal_source(owner)
            .map(NominalSourceContractV1::source_shape)
            .map_err(Into::into)
    }
}
