use super::*;

impl DecodedTypeFoundationSourceAuthorityV1 {
    pub fn resolve<R: TypeFoundationSourceResolver<E>, E: fmt::Display>(
        self,
        resolver: &mut R,
    ) -> Result<TypeFoundationSourceAuthorityV1, TypeFoundationSourceError> {
        let provider = resolver
            .resolve(self.provider)
            .map_err(|e| constituent(1, e))?;
        let exact_keys = self
            .exact_keys
            .resolve(resolver)
            .map_err(|e| constituent(2, e))?;
        let sources = self
            .sources
            .resolve(resolver)
            .map_err(|e| inventory(3, e))?;
        let representations = self
            .representations
            .resolve(resolver)
            .map_err(|e| match e {
                NominalRepresentationTableResolutionError::Resource(error) => {
                    TypeFoundationSourceError::Resource(error)
                }
                error => constituent(4, error),
            })?;
        let generated_nominals = self
            .generated_nominals
            .resolve(resolver)
            .map_err(|e| constituent(5, e))?;
        let accessor_keys = self
            .accessor_keys
            .resolve(resolver)
            .map_err(|e| constituent(6, e))?;
        let definition_sources =
            self.definition_sources
                .resolve(resolver)
                .map_err(|e| match e {
                    ExportDefinitionSourceSetValidationError::Allocation(error) => {
                        TypeFoundationSourceError::Resource(error)
                    }
                    error => constituent(7, error),
                })?;
        let source_roots = self
            .source_roots
            .resolve(resolver)
            .map_err(|e| inventory(8, e))?;
        let local_exact_facts = self
            .local_exact_facts
            .resolve(resolver)
            .map_err(|e| constituent(9, e))?;
        let dependency_facts = self
            .dependency_facts
            .resolve(resolver)
            .map_err(|e| inventory(10, e))?;
        let local_inheritance_edges = self
            .local_inheritance_edges
            .resolve(resolver)
            .map_err(|e| inventory(11, e))?;
        let fact_shapes = self.fact_shapes.resolve(resolver).map_err(|e| match e {
            TypeFactShapeSourceError::Resource(error) => TypeFoundationSourceError::Resource(error),
            error => constituent(12, error),
        })?;
        let representation_owners = self
            .representation_owners
            .resolve(resolver)
            .map_err(|e| constituent(13, e))?;
        TypeFoundationSourceAuthorityV1::try_new(TypeFoundationSourceEntriesV1 {
            provider,
            exact_keys,
            sources,
            representations,
            generated_nominals,
            accessor_keys,
            definition_sources,
            source_roots,
            local_exact_facts,
            dependency_facts,
            local_inheritance_edges,
            fact_shapes,
            representation_owners,
        })
    }
}

fn constituent(field: u64, error: impl fmt::Display) -> TypeFoundationSourceError {
    TypeFoundationSourceError::Constituent {
        field,
        reason: error.to_string(),
    }
}

fn inventory(field: u64, error: SourceInventoryError) -> TypeFoundationSourceError {
    match error {
        SourceInventoryError::Resource(error) => TypeFoundationSourceError::Resource(error),
        error => constituent(field, error),
    }
}
