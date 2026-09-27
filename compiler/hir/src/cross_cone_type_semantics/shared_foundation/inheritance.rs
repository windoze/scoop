//! Scoped reuse of the ordinary inheritance graph and access-domain replay.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use scoop_identity::{ExactTypeKey, GeneratedNominalKey, SourceDeclarationKey};
use scoop_wire::WirePath;

use super::*;
use crate::{
    CheckedNominalInheritanceGraphV1, DeclarationAccessSourceV1, ExportDefinitionSourceV1,
    NominalInheritanceEdgesV1, NominalInheritanceSemanticAuthority, NominalRepresentationSupportV1,
    SourceNominalId,
};

mod contracts;
mod edges;
mod members;
mod schemas;
mod source;

impl SharedTypeMetadataV1<'_> {
    pub(crate) fn callable_declaration_access(
        self,
        source: &crate::CallableDeclarationRecordV1,
    ) -> Result<DeclarationAccessSourceV1, Error> {
        contracts::callable_access(self, source)
    }
}

impl CheckedSharedTypeFoundationV1<'_> {
    /// Replays every supplied provider's complete inheritance inventory against
    /// its shared declarations. The temporary graph cannot escape this scope.
    pub fn with_inheritance_graph<R>(
        self,
        dependencies: &[CheckedSharedTypeFoundationV1<'_>],

        use_graph: impl for<'graph> FnOnce(&CheckedNominalInheritanceGraphV1<'graph>) -> R,
    ) -> Result<R, Error> {
        let types = MetadataTypes {
            current: self.metadata,
            dependencies,
        };
        types.validate_dependencies()?;
        let mut context = Context::default();
        for provider in std::iter::once(self).chain(dependencies.iter().copied()) {
            source::collect(&mut context, provider)?;
            edges::collect(&mut context, provider, dependencies)?;
        }
        let graph = CheckedNominalInheritanceGraphV1::validate_with_source_roots(
            context.edges.iter(),
            context.sources.keys().copied(),
            &context,
        )
        .map_err(|error| Error::InheritanceGraph(Box::new(error)))?;
        for provider in std::iter::once(self).chain(dependencies.iter().copied()) {
            for record in provider.section.inheritance().records() {
                let owner = graph
                    .get(record.owner())
                    .ok_or(Error::InheritanceEdges(record.owner()))?
                    .source();

                let declaration = provider
                    .metadata
                    .public
                    .nominal_interfaces()
                    .declaration(owner)
                    .ok_or(Error::InheritanceSource(owner))?;
                members::validate(provider, declaration, record, &context)?;
            }
        }
        schemas::validate(self, dependencies, &context, &graph)?;
        Ok(use_graph(&graph))
    }
}

#[derive(Default)]
struct Context<'a> {
    sources: BTreeMap<SourceNominalId, Source>,
    origins: BTreeSet<ExportDefinitionSourceV1>,
    exacts: BTreeMap<PersistentExactTypeId, Arc<ExactTypeKey>>,
    generated: BTreeMap<PersistentTypeId, Arc<GeneratedNominalKey>>,
    objects: BTreeMap<PersistentTypeId, &'a NominalRepresentationSupportV1>,
    edges: Vec<NominalInheritanceEdgesV1>,
}

struct Source {
    key: Arc<SourceDeclarationKey>,
    access: DeclarationAccessSourceV1,
}

impl Context<'_> {
    fn source(&self, owner: SourceNominalId) -> Result<&Source, Error> {
        self.sources
            .get(&owner)
            .ok_or(Error::InheritanceSource(owner))
    }
}

impl NominalInheritanceSemanticAuthority<Error> for Context<'_> {
    fn exact_type_key(&self, exact: PersistentExactTypeId) -> Result<&ExactTypeKey, Error> {
        self.exacts
            .get(&exact)
            .map(AsRef::as_ref)
            .ok_or(Error::MissingFact(exact))
    }
    fn nominal_declaration_key(
        &self,
        owner: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, Error> {
        Ok(&self.source(owner)?.key)
    }
    fn nominal_access_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&DeclarationAccessSourceV1, Error> {
        Ok(&self.source(owner)?.access)
    }
    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, Error> {
        Ok(self.source(owner)?.access.definition_origin())
    }
    fn validate_definition_source(&self, source: &ExportDefinitionSourceV1) -> Result<(), Error> {
        if self.origins.contains(source) {
            Ok(())
        } else {
            Err(Error::InheritanceOrigin)
        }
    }
    fn object_representation(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&NominalRepresentationSupportV1, Error> {
        self.objects
            .get(&owner)
            .copied()
            .ok_or(Error::Representation(owner))
    }
    fn generated_nominal_key(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&GeneratedNominalKey, Error> {
        self.generated
            .get(&owner)
            .map(AsRef::as_ref)
            .ok_or(Error::Representation(owner))
    }
}
