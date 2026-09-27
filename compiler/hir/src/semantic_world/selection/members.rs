//! Source member candidates share the ordinary callable catalog and defaults.

use std::collections::BTreeMap;
use std::sync::Arc;

use scoop_identity::CanonicalIdentifier;

use super::intrinsics::CallableCatalogName;
use super::{
    ImportedDependencyCallableCandidate, ImportedDependencyCandidateError,
    ImportedDependencyDefinitionSource, ImportedDependencyDefinitionSources,
    ImportedDependencySelectionPlan,
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
}

impl ImportedCallableSource for ImportedDependencyCallableCandidate {
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
    interface: CallableDeclarationRecordV1,
    capability: Option<ParamFreeNominalCallableV1>,
    source: Option<CallableSourceInterfaceV1>,
    defaults: BTreeMap<ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1>,
    definition_sources: Arc<ImportedDependencyDefinitionSources>,
    callable_body: Option<Arc<crate::ExportGenericCallableBodyV1>>,
}

impl ImportedCallableDeclaration {
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub const fn capability(&self) -> Option<&ParamFreeNominalCallableV1> {
        self.capability.as_ref()
    }
}

impl ImportedCallableSource for ImportedCallableDeclaration {
    fn callable_body(&self) -> Option<&crate::ExportGenericCallableBodyV1> {
        self.callable_body.as_deref()
    }
    fn interface(&self) -> &CallableDeclarationRecordV1 {
        &self.interface
    }
    fn source_interface(&self) -> Option<&CallableSourceInterfaceV1> {
        self.source.as_ref()
    }
    fn default_template(
        &self,
        key: ExportDefaultTemplateKeyV1,
    ) -> Option<&ExportDefaultTemplateV1> {
        self.defaults.get(&key)
    }
    fn definition_source(
        &self,
        source: &ExportDefinitionSourceV1,
    ) -> Option<ImportedDependencyDefinitionSource<'_>> {
        self.definition_sources.resolve(source)
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
            .callables
            .values()
            .find(|entry| {
                entry.interface.owner() == PublicDeclarationOwnerV1::Nominal(owner)
                    && entry.interface.slot_relations().values().contains(&slot)
            })
            .map(|entry| self.callable_declaration(entry.interface.declaration()))
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
                &self
                    .catalog
                    .properties
                    .values()
                    .find(|property| {
                        let accessor = property.interface.accessors();
                        declaration
                            == scoop_identity::CallableTemplateOrigin::Accessor(accessor.getter())
                            || accessor.setter().is_some_and(|setter| {
                                declaration
                                    == scoop_identity::CallableTemplateOrigin::Accessor(setter)
                            })
                    })
                    .ok_or(ImportedDependencyCandidateError::MissingCallableSource(
                        declaration,
                    ))?
                    .name
            }
            CallableCatalogName::Constructor => {
                let PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(owner)) =
                    entry.interface.owner()
                else {
                    return Err(ImportedDependencyCandidateError::MissingCallableSource(
                        declaration,
                    ));
                };
                let nominal = self.catalog.nominals.get(&owner).ok_or(
                    ImportedDependencyCandidateError::MissingCallableSource(declaration),
                )?;
                let scoop_identity::DeclarationName::Named(name) = nominal.identity.key().name()
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
            interface: entry.interface.clone(),
            capability: entry.capability.clone(),
            source: entry.source.clone(),
            defaults: entry.default_templates.clone(),
            definition_sources: Arc::clone(&entry.definition_sources),
            callable_body: entry.callable_body.clone(),
        })
    }

    pub fn constructor_candidates(
        &self,
        owner: scoop_identity::PersistentTypeId,
    ) -> Result<Vec<ImportedCallableDeclaration>, ImportedDependencyCandidateError> {
        self.catalog
            .callables
            .values()
            .filter(|entry| {
                entry.interface.owner()
                    == PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(owner))
                    && matches!(entry.name, CallableCatalogName::Constructor)
            })
            .map(|entry| self.callable_declaration(entry.interface.declaration()))
            .collect()
    }

    pub fn member_callable_candidates(
        &self,
        owner: SourceNominalId,
        lookup: ImportedMemberLookup<'_>,
    ) -> Result<Vec<ImportedCallableDeclaration>, ImportedDependencyCandidateError> {
        let mut candidates = Vec::new();
        let property = match lookup {
            ImportedMemberLookup::PropertyGetter(name)
            | ImportedMemberLookup::PropertySetter(name) => {
                self.catalog.properties.values().find(|property| {
                    property.interface.owner() == PublicDeclarationOwnerV1::Nominal(owner)
                        && property.name.as_str() == name
                })
            }
            _ => None,
        };
        for entry in self.catalog.callables.values() {
            if entry.interface.owner() != PublicDeclarationOwnerV1::Nominal(owner) {
                continue;
            }
            let name = match (&entry.name, lookup) {
                (CallableCatalogName::Function(name), ImportedMemberLookup::Name(expected))
                    if name.as_str() == expected =>
                {
                    name
                }
                (CallableCatalogName::Function(name), ImportedMemberLookup::Operator(expected))
                    if entry.interface.effects().operator_role() == expected =>
                {
                    name
                }
                (
                    CallableCatalogName::Accessor,
                    ImportedMemberLookup::PropertyGetter(_)
                    | ImportedMemberLookup::PropertySetter(_),
                ) => {
                    let Some(property) = property else {
                        continue;
                    };
                    let accessor = match lookup {
                        ImportedMemberLookup::PropertyGetter(_) => {
                            Some(property.interface.accessors().getter())
                        }
                        ImportedMemberLookup::PropertySetter(_) => {
                            property.interface.accessors().setter()
                        }
                        _ => unreachable!("an accessor lookup has a property role"),
                    };
                    if Some(entry.interface.declaration())
                        != accessor.map(scoop_identity::CallableTemplateOrigin::Accessor)
                    {
                        continue;
                    }
                    &property.name
                }
                _ => continue,
            };
            if matches!(entry.name, CallableCatalogName::Function(_)) && entry.source.is_none() {
                return Err(ImportedDependencyCandidateError::MissingCallableSource(
                    entry.interface.declaration(),
                ));
            }
            candidates.push(ImportedCallableDeclaration {
                name: name.clone(),
                interface: entry.interface.clone(),
                capability: entry.capability.clone(),
                source: entry.source.clone(),
                defaults: entry.default_templates.clone(),
                definition_sources: Arc::clone(&entry.definition_sources),
                callable_body: entry.callable_body.clone(),
            });
        }
        Ok(candidates)
    }
}
