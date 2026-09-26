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
    CallableInterfaceRecordV1, CallableOperatorRoleV1, CallableSourceInterfaceV1,
    ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1, ExportDefinitionSourceV1,
    ParamFreeNominalCallableV1, PublicDeclarationOwnerV1, SourceNominalId,
};

/// Common source metadata used while probing and materializing a call.
pub trait ImportedCallableSource {
    fn interface(&self) -> &CallableInterfaceRecordV1;
    fn source_interface(&self) -> Option<&CallableSourceInterfaceV1>;
    fn default_template(&self, key: ExportDefaultTemplateKeyV1)
    -> Option<&ExportDefaultTemplateV1>;
    fn definition_source(
        &self,
        source: &ExportDefinitionSourceV1,
    ) -> Option<ImportedDependencyDefinitionSource<'_>>;
}

impl ImportedCallableSource for ImportedDependencyCallableCandidate {
    fn interface(&self) -> &CallableInterfaceRecordV1 {
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
}

/// A callable referenced by its actual declaration, including member and default uses.
#[derive(Clone, Debug)]
pub struct ImportedCallableDeclaration {
    name: CanonicalIdentifier,
    interface: CallableInterfaceRecordV1,
    capability: Option<ParamFreeNominalCallableV1>,
    source: Option<CallableSourceInterfaceV1>,
    defaults: BTreeMap<ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1>,
    definition_sources: Arc<ImportedDependencyDefinitionSources>,
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
    fn interface(&self) -> &CallableInterfaceRecordV1 {
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
                        let accessor = property.interface.capability();
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
            ImportedMemberLookup::PropertyGetter(name) => {
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
                (CallableCatalogName::Accessor, ImportedMemberLookup::PropertyGetter(_)) => {
                    let Some(property) = property else {
                        continue;
                    };
                    if entry.interface.declaration()
                        != scoop_identity::CallableTemplateOrigin::Accessor(
                            property.interface.capability().getter(),
                        )
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
            });
        }
        Ok(candidates)
    }
}
