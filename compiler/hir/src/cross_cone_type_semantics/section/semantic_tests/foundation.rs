use super::*;

impl NominalInheritanceSemanticAuthority<&'static str> for Fixture {
    fn exact_type_key(&self, id: PersistentExactTypeId) -> Result<&ExactTypeKey, &'static str> {
        self.source.exact_type_key(id)
    }
    fn nominal_declaration_key(
        &self,
        id: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, &'static str> {
        NominalInheritanceSemanticAuthority::nominal_declaration_key(&self.source, id)
    }
    fn nominal_access_source(
        &self,
        id: SourceNominalId,
    ) -> Result<&DeclarationAccessSourceV1, &'static str> {
        self.source.nominal_access_source(id)
    }
    fn nominal_definition_source(
        &self,
        id: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, &'static str> {
        self.source.nominal_definition_source(id)
    }
    fn validate_definition_source(
        &self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), &'static str> {
        self.source.validate_definition_source(source)
    }
    fn object_representation(
        &self,
        id: PersistentTypeId,
    ) -> Result<&NominalRepresentationSupportV1, &'static str> {
        self.source.object_representation(id)
    }
    fn generated_nominal_key(
        &self,
        id: PersistentTypeId,
    ) -> Result<&GeneratedNominalKey, &'static str> {
        self.source.generated_nominal_key(id)
    }
}
impl ExactTypeFactsSemanticAuthority<&'static str> for Fixture {
    fn fact_shape(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ExactTypeFactShapeV1, &'static str> {
        self.shapes.get(&exact).ok_or("unknown source fact shape")
    }
}
impl NominalRepresentationSemanticAuthority<&'static str> for Fixture {
    fn current_provider(&self) -> ConeIdentity {
        self.provider
    }
    fn required_representation_owners(
        &self,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentTypeId>, &'static str> {
        Ok(&self.representations)
    }
    fn representation_source(
        &self,
        owner: PersistentTypeId,
    ) -> Result<NominalRepresentationSourceV1<'_>, &'static str> {
        let source = SourceNominalId::Concrete(owner);
        Ok(NominalRepresentationSourceV1 {
            key: self
                .source
                .graph
                .keys
                .get(&source)
                .ok_or("unknown source")?,
            access: self
                .source
                .graph
                .access
                .get(&source)
                .ok_or("unknown source access")?,
            shape: self
                .source
                .graph
                .representations
                .get(&owner)
                .ok_or("unknown source representation")?
                .shape(),
            public_source_shape: NominalRepresentationPublicSourceShapeV1::NoPublicSourceShape,
        })
    }
}
impl TypeSectionFoundationSemanticAuthority<&'static str> for Fixture {
    fn local_exact_facts(
        &self,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentExactTypeId>, &'static str> {
        Ok(&self.facts)
    }
    fn dependency_facts(&self) -> Result<&[TypeSectionDependencyFactV1], &'static str> {
        Ok(&self.foreign)
    }
    fn local_source_roots(&self) -> Result<&[SourceNominalId], &'static str> {
        Ok(&self.roots)
    }
    fn local_inheritance_edges(&self) -> Result<&[NominalInheritanceEdgesV1], &'static str> {
        Ok(&self.edges)
    }
    fn selected_accessor_key(
        &self,
        id: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, &'static str> {
        self.source.accessors.get(&id).ok_or("unknown accessor")
    }
}
