use super::*;

impl CrossConeTypeSemanticsFoundationV1 {
    /// Serializes only the independent source projection. The candidate
    /// type-semantics section is neither read nor required by this operation.
    pub fn source_transcript(
        &self,
    ) -> Result<TypeFoundationSourceAuthorityV1, TypeFoundationSourceError> {
        let exact_keys =
            CanonicalPersistentIdsV1::try_new(self.exact_keys.keys().copied().collect())
                .map_err(|error| constituent(2, error))?;
        let sources = CanonicalTypeSourceNominalsV1::try_new(
            self.source_nominals()
                .map(|(owner, access)| TypeSourceNominalV1::new(owner, access.clone()))
                .collect(),
        )
        .map_err(|error| constituent(3, error))?;
        let representations = self
            .representations
            .iter()
            .map(|(owner, evidence)| {
                let source = self
                    .sources
                    .get(&SourceNominalId::Concrete(*owner))
                    .ok_or(TypeFoundationSourceError::MissingSourceOwner(*owner))?;
                NominalRepresentationSupportV1::try_new(
                    &source.key,
                    source.access.clone(),
                    evidence.shape.clone(),
                )
                .map_err(|error| constituent(4, error))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let representations = CanonicalNominalRepresentationSupportV1::try_new(representations)
            .map_err(|error| constituent(4, error))?;
        let generated_nominals =
            CanonicalPersistentIdsV1::try_new(self.generated_nominals.keys().copied().collect())
                .map_err(|error| constituent(5, error))?;
        let accessor_keys =
            CanonicalPersistentIdsV1::try_new(self.accessor_keys.keys().copied().collect())
                .map_err(|error| constituent(6, error))?;
        let definition_sources = CanonicalExportDefinitionSourcesV1::try_new(
            self.valid_definition_sources.iter().cloned().collect(),
        )
        .map_err(|error| constituent(7, error))?;
        let source_roots = CanonicalSourceNominalIdsV1::try_new(self.source_roots.clone())
            .map_err(|error| constituent(8, error))?;
        let dependency_facts =
            CanonicalTypeSectionDependencyFactsV1::try_new(self.dependency_facts.clone())
                .map_err(|error| constituent(10, error))?;
        let local_inheritance_edges =
            CanonicalNominalInheritanceEdgesV1::try_new(self.local_inheritance_edges.clone())
                .map_err(|error| constituent(11, error))?;
        let fact_shapes = CanonicalExactTypeFactShapesV1::try_new(
            self.source_fact_shapes()
                .map(|(exact, shape)| ExactTypeFactShapeRecordV1::new(exact, shape.clone()))
                .collect(),
        )
        .map_err(|error| constituent(12, error))?;
        TypeFoundationSourceAuthorityV1::try_new(TypeFoundationSourceEntriesV1 {
            provider: self.provider,
            exact_keys,
            sources,
            representations,
            generated_nominals,
            accessor_keys,
            definition_sources,
            source_roots,
            local_exact_facts: self.local_exact_facts.clone(),
            dependency_facts,
            local_inheritance_edges,
            fact_shapes,
            representation_owners: self.representation_owners.clone(),
        })
    }
}

fn constituent(field: u64, error: impl fmt::Display) -> TypeFoundationSourceError {
    TypeFoundationSourceError::Constituent {
        field,
        reason: error.to_string(),
    }
}
