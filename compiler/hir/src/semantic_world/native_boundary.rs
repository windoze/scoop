//! Derives native source witnesses from the actual dependency declaration provider.

use scoop_identity::{SourceDeclarationKey, SourceDeclarationKind};

use super::{ImportedProvider, ImportedSemanticWorld};
use crate::{
    HirNativeBoundaryTypeDefinitionError as Error, NativeBoundaryNominalOwner as Owner,
    NativeBoundaryNominalShape as Shape, NativeBoundaryTypeDefinitionRecord as Record,
};

mod shapes;

impl ImportedSemanticWorld<'_> {
    pub(crate) fn native_boundary_type_definition(
        &self,
        owner: Owner,
        project: impl FnOnce(Record) -> Result<Record, Error>,
    ) -> Result<Record, Error> {
        let (provider, declaration) = self
            .native_declaration_provider(owner)
            .ok_or(Error::MissingSourceNominal { owner })?;
        let existing = provider.foundation().native_boundary_type(owner);
        let record = if let Some(source) = provider
            .interface()
            .nominal_interfaces()
            .get(owner.declaration_owner())
        {
            let shape = shapes::project(provider, source.source_shape())?;
            Record::new(declaration, &[source.type_parameters().len_u32()], shape)
                .and_then(|record| {
                    record.with_c_abi(existing.map_or(
                        crate::NativeBoundaryCAbiV1::SourceRepresentation,
                        Record::c_abi,
                    ))
                })
                .map_err(Error::InvalidDefinition)?
        } else if let Some(existing) = existing {
            // This is an exact reference to an already validated witness, not public lookup.
            existing.clone()
        } else if matches!(
            declaration.declaration_kind(),
            SourceDeclarationKind::Class
                | SourceDeclarationKind::Interface
                | SourceDeclarationKind::Object
                | SourceDeclarationKind::AnnotationClass
        ) {
            Record::new(
                declaration,
                &[declaration.duplicate_signature().type_parameter_count()],
                Shape::Reference,
            )
            .map_err(Error::InvalidDefinition)?
        } else {
            return Err(Error::MissingDependencyShape { owner });
        };
        let record = project(record)?;
        if existing.is_some_and(|existing| existing != &record) {
            return Err(Error::DependencyDefinitionMismatch { owner });
        }
        Ok(record)
    }

    fn native_declaration_provider(
        &self,
        owner: Owner,
    ) -> Option<(&ImportedProvider<'_>, &SourceDeclarationKey)> {
        self.providers.iter().find_map(|provider| {
            let foundation = provider.foundation().canonical_for_semantic_authority();
            let key = match owner {
                Owner::Concrete(id) => foundation.source_type_key(id),
                Owner::GenericTemplate(id) => foundation
                    .generic_type_by_bytes(id.as_array())
                    .map(|(_, key)| key),
            }?;
            // Borrowed witnesses in another artifact never change declaration ownership.
            (key.origin() == provider.identity()).then_some((provider, key))
        })
    }
}
