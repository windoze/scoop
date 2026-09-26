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

/// A member of an exact source nominal, without a fabricated import binding.
#[derive(Clone, Debug)]
pub struct ImportedMemberCallableCandidate {
    name: CanonicalIdentifier,
    interface: CallableInterfaceRecordV1,
    capability: Option<ParamFreeNominalCallableV1>,
    source: Option<CallableSourceInterfaceV1>,
    defaults: BTreeMap<ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1>,
    definition_sources: Arc<ImportedDependencyDefinitionSources>,
}

impl ImportedMemberCallableCandidate {
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub const fn capability(&self) -> Option<&ParamFreeNominalCallableV1> {
        self.capability.as_ref()
    }
}

impl ImportedCallableSource for ImportedMemberCallableCandidate {
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
    pub fn member_callable_candidates(
        &self,
        owner: SourceNominalId,
        lookup: ImportedMemberLookup<'_>,
    ) -> Result<Vec<ImportedMemberCallableCandidate>, ImportedDependencyCandidateError> {
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
            candidates.push(ImportedMemberCallableCandidate {
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
