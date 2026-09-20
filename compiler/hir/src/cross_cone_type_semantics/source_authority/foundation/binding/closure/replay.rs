use super::*;

impl NominalInheritanceSemanticAuthority<TypeFoundationReplayError>
    for TypeFoundationSourceClosureV1<'_>
{
    fn exact_type_key(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ExactTypeKey, TypeFoundationReplayError> {
        self.exacts
            .get(&exact)
            .copied()
            .ok_or_else(|| TypeFoundationBindingError::MissingExact(exact).into())
    }

    fn nominal_declaration_key(
        &self,
        owner: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, TypeFoundationReplayError> {
        Ok(self.nominal(owner)?.source.nominal_key(owner)?)
    }

    fn nominal_access_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&DeclarationAccessSourceV1, TypeFoundationReplayError> {
        Ok(self.nominal(owner)?.source.nominal_source(owner)?.access())
    }

    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, TypeFoundationReplayError> {
        self.nominal_access_source(owner)
            .map(DeclarationAccessSourceV1::definition_origin)
    }

    fn validate_definition_source(
        &self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), TypeFoundationReplayError> {
        let provider = source.origin().source().cone();
        let bound = self
            .providers
            .get(&provider)
            .ok_or(TypeFoundationReplayError::MissingProvider(provider))?
            .source;
        Ok(NominalInheritanceSemanticAuthority::validate_definition_source(bound, source)?)
    }

    fn object_representation(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&NominalRepresentationSupportV1, TypeFoundationReplayError> {
        Ok(self
            .nominal(SourceNominalId::Concrete(owner))?
            .source
            .object_representation(owner)?)
    }

    fn generated_nominal_key(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&GeneratedNominalKey, TypeFoundationReplayError> {
        self.generated
            .get(&owner)
            .copied()
            .ok_or_else(|| TypeFoundationBindingError::MissingGenerated(owner).into())
    }
}

impl ExactTypeFactsSemanticAuthority<TypeFoundationReplayError>
    for TypeFoundationSourceClosureV1<'_>
{
    fn fact_shape(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ExactTypeFactShapeV1, TypeFoundationReplayError> {
        self.facts
            .get(&exact)
            .map(|(_, shape)| *shape)
            .ok_or_else(|| TypeFoundationBindingError::MissingFactShape(exact).into())
    }
}

impl NominalRepresentationSemanticAuthority<TypeFoundationReplayError>
    for TypeFoundationSourceClosureV1<'_>
{
    fn current_provider(&self) -> ConeIdentity {
        self.root.provider()
    }

    fn required_representation_owners(
        &self,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentTypeId>, TypeFoundationReplayError> {
        Ok(&self.root.entries().representation_owners)
    }

    fn representation_source(
        &self,
        owner: PersistentTypeId,
    ) -> Result<NominalRepresentationSourceV1<'_>, TypeFoundationReplayError> {
        self.nominal(SourceNominalId::Concrete(owner))?
            .representation(owner)
    }
}

impl TypeSectionFoundationSemanticAuthority<TypeFoundationReplayError>
    for TypeFoundationSourceClosureV1<'_>
{
    fn local_exact_facts(
        &self,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentExactTypeId>, TypeFoundationReplayError> {
        Ok(&self.root.entries().local_exact_facts)
    }

    fn dependency_facts(
        &self,
    ) -> Result<&[TypeSectionDependencyFactV1], TypeFoundationReplayError> {
        Ok(self.root.entries().dependency_facts.records())
    }

    fn local_source_roots(&self) -> Result<&[SourceNominalId], TypeFoundationReplayError> {
        Ok(self.root.entries().source_roots.values())
    }

    fn local_inheritance_edges(
        &self,
    ) -> Result<&[NominalInheritanceEdgesV1], TypeFoundationReplayError> {
        Ok(self.root.entries().local_inheritance_edges.records())
    }

    fn selected_accessor_key(
        &self,
        accessor: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, TypeFoundationReplayError> {
        self.accessors
            .get(&accessor)
            .copied()
            .ok_or_else(|| TypeFoundationBindingError::MissingAccessor(accessor).into())
    }
}
