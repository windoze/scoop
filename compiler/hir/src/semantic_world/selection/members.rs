//! Source member candidates share the ordinary callable catalog and defaults.

mod context;
use std::sync::Arc;

use scoop_identity::CanonicalIdentifier;

use super::catalog::CallableCatalogEntry;
use super::intrinsics::CallableCatalogName;
use super::{
    ImportedDependencyCallableCandidate, ImportedDependencyCandidateError,
    ImportedDependencyDefinitionSource, ImportedDependencySelectionPlan,
};
use crate::{
    CallableDeclarationRecordV1, CallableOperatorRoleV1, CallableSourceInterfaceV1,
    ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1, ExportDefinitionSourceV1,
    ParamFreeNominalCallableV1, PublicDeclarationOwnerV1, SourceNominalId,
};

/// Common source metadata used while probing and materializing a call.
pub trait ImportedCallableSource {
    fn interface(&self) -> &CallableDeclarationRecordV1;
    fn source_interface(&self) -> Option<&CallableSourceInterfaceV1>;
    fn callable_body(&self) -> Option<&crate::ExportGenericCallableBodyV1>;
    fn default_template(&self, key: ExportDefaultTemplateKeyV1)
    -> Option<&ExportDefaultTemplateV1>;
    fn definition_source(
        &self,
        source: &ExportDefinitionSourceV1,
    ) -> Option<ImportedDependencyDefinitionSource<'_>>;
    fn source_location(
        &self,
        source: &scoop_identity::SourceIdentity,
        context: scoop_identity::PersistentSourceContextId,
    ) -> Option<ImportedDependencyDefinitionSource<'_>>;
}

impl ImportedCallableSource for ImportedDependencyCallableCandidate {
    fn source_location(
        &self,
        source: &scoop_identity::SourceIdentity,
        context: scoop_identity::PersistentSourceContextId,
    ) -> Option<ImportedDependencyDefinitionSource<'_>> {
        self.entry
            .definition_sources
            .resolve_location(source, context)
    }
    fn callable_body(&self) -> Option<&crate::ExportGenericCallableBodyV1> {
        self.callable_body()
    }
    fn interface(&self) -> &CallableDeclarationRecordV1 {
        self.interface()
    }
    fn source_interface(&self) -> Option<&CallableSourceInterfaceV1> {
        self.source_interface()
    }
    fn default_template(
        &self,
        key: ExportDefaultTemplateKeyV1,
    ) -> Option<&ExportDefaultTemplateV1> {
        self.default_template(key)
    }
    fn definition_source(
        &self,
        source: &ExportDefinitionSourceV1,
    ) -> Option<ImportedDependencyDefinitionSource<'_>> {
        self.definition_source(source)
    }
}

#[derive(Clone, Copy)]
pub enum ImportedMemberLookup<'a> {
    Name(&'a str),
    Operator(CallableOperatorRoleV1),
    PropertyGetter(&'a str),
    PropertySetter(&'a str),
}

/// A callable referenced by its actual declaration, including member and default uses.
#[derive(Clone, Debug)]
pub struct ImportedCallableDeclaration {
    name: CanonicalIdentifier,
    entry: Arc<CallableCatalogEntry>,
}

impl ImportedCallableDeclaration {
    pub fn source_context(
        &self,
        key: &scoop_identity::SourceContextKey,
    ) -> Option<&scoop_identity::SourceContextKey> {
        let id = scoop_identity::PersistentSourceContextId::from_key(key).ok()?;
        self.entry
            .definition_sources
            .contexts
            .get(&id)
            .map(|(key, _)| key)
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn capability(&self) -> Option<&ParamFreeNominalCallableV1> {
        self.entry.capability.as_ref()
    }

    pub fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.entry.definition_origin
    }
}

impl ImportedCallableSource for ImportedCallableDeclaration {
    fn source_location(
        &self,
        source: &scoop_identity::SourceIdentity,
        context: scoop_identity::PersistentSourceContextId,
    ) -> Option<ImportedDependencyDefinitionSource<'_>> {
        self.entry
            .definition_sources
            .resolve_location(source, context)
    }
    fn callable_body(&self) -> Option<&crate::ExportGenericCallableBodyV1> {
        self.entry.callable_body.as_deref()
    }
    fn interface(&self) -> &CallableDeclarationRecordV1 {
        &self.entry.interface
    }
    fn source_interface(&self) -> Option<&CallableSourceInterfaceV1> {
        self.entry.source.as_ref()
    }
    fn default_template(
        &self,
        key: ExportDefaultTemplateKeyV1,
    ) -> Option<&ExportDefaultTemplateV1> {
        self.entry.default_templates.get(&key)
    }
    fn definition_source(
        &self,
        source: &ExportDefinitionSourceV1,
    ) -> Option<ImportedDependencyDefinitionSource<'_>> {
        self.entry.definition_sources.resolve(source)
    }
}

impl ImportedDependencySelectionPlan {
    pub fn callable_source_interface(
        &self,
        declaration: scoop_identity::CallableTemplateOrigin,
    ) -> Option<&CallableSourceInterfaceV1> {
        self.catalog.callables.get(&declaration)?.source.as_ref()
    }

    pub fn callable_for_slot(
        &self,
        owner: SourceNominalId,
        slot: scoop_identity::PersistentDispatchSlotId,
    ) -> Result<Option<ImportedCallableDeclaration>, ImportedDependencyCandidateError> {
        self.catalog
            .indexes
            .nominals
            .get(&owner)
            .and_then(|members| members.slots.get(&slot))
            .map(|&declaration| self.callable_declaration(declaration))
            .transpose()
    }

    pub fn callable_declaration(
        &self,
        declaration: scoop_identity::CallableTemplateOrigin,
    ) -> Result<ImportedCallableDeclaration, ImportedDependencyCandidateError> {
        let entry = self.catalog.callables.get(&declaration).ok_or(
            ImportedDependencyCandidateError::MissingCallable(declaration),
        )?;
        let name = match &entry.name {
            CallableCatalogName::Function(name) | CallableCatalogName::VariantConstructor(name) => {
                name
            }
            CallableCatalogName::Accessor => {
                let scoop_identity::CallableTemplateOrigin::Accessor(accessor) = declaration else {
                    unreachable!("the catalog retains the callable declaration kind");
                };
                let property = self.catalog.indexes.accessors.get(&accessor).ok_or(
                    ImportedDependencyCandidateError::MissingCallableSource(declaration),
                )?;
                &self.catalog.properties[property].name
            }
            CallableCatalogName::Constructor => {
                let PublicDeclarationOwnerV1::Nominal(owner) = entry.interface.owner() else {
                    return Err(ImportedDependencyCandidateError::MissingCallableSource(
                        declaration,
                    ));
                };
                let nominal = self.catalog.nominals.get(&owner).ok_or(
                    ImportedDependencyCandidateError::MissingCallableSource(declaration),
                )?;
                let scoop_identity::DeclarationName::Named(name) =
                    nominal.identity.declaration().name()
                else {
                    return Err(ImportedDependencyCandidateError::MissingCallableSource(
                        declaration,
                    ));
                };
                name
            }
        };
        Ok(ImportedCallableDeclaration {
            name: name.clone(),
            entry: Arc::clone(entry),
        })
    }

    pub fn constructor_candidates(
        &self,
        owner: SourceNominalId,
    ) -> Result<Vec<ImportedCallableDeclaration>, ImportedDependencyCandidateError> {
        self.catalog
            .indexes
            .nominals
            .get(&owner)
            .into_iter()
            .flat_map(|members| &members.constructors)
            .map(|&declaration| self.callable_declaration(declaration))
            .collect()
    }

    pub fn member_callable_candidates(
        &self,
        owner: SourceNominalId,
        lookup: ImportedMemberLookup<'_>,
    ) -> Result<Vec<ImportedCallableDeclaration>, ImportedDependencyCandidateError> {
        let Some(members) = self.catalog.indexes.nominals.get(&owner) else {
            return Ok(Vec::new());
        };
        let accessor;
        let declarations = match lookup {
            ImportedMemberLookup::Name(name) => members
                .functions
                .get(name)
                .map(Vec::as_slice)
                .unwrap_or_default(),
            ImportedMemberLookup::Operator(role) => members
                .operators
                .get(&role)
                .map(Vec::as_slice)
                .unwrap_or_default(),
            ImportedMemberLookup::PropertyGetter(name)
            | ImportedMemberLookup::PropertySetter(name) => {
                accessor = members
                    .property_by_name
                    .get(name)
                    .and_then(|property| {
                        let property = &self.catalog.properties[property];
                        let accessors = property.interface.accessors();
                        match lookup {
                            ImportedMemberLookup::PropertyGetter(_) => Some(accessors.getter()),
                            ImportedMemberLookup::PropertySetter(_) => accessors.setter(),
                            _ => unreachable!("an accessor lookup has a property role"),
                        }
                    })
                    .map(scoop_identity::CallableTemplateOrigin::Accessor);
                accessor.as_slice()
            }
        };
        let mut candidates = Vec::with_capacity(declarations.len());
        for &declaration in declarations {
            let Some(entry) = self.catalog.callables.get(&declaration) else {
                continue;
            };
            if entry.interface.owner() != PublicDeclarationOwnerV1::Nominal(owner) {
                continue;
            }
            if matches!(entry.name, CallableCatalogName::Function(_)) && entry.source.is_none() {
                return Err(ImportedDependencyCandidateError::MissingCallableSource(
                    declaration,
                ));
            }
            candidates.push(self.callable_declaration(declaration)?);
        }
        Ok(candidates)
    }
}
