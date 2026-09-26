use std::collections::BTreeMap;
use std::sync::Arc;

use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, PersistentPropertyId, PersistentTypeAliasId,
    PropertyOwner,
};

use super::{
    ImportedDependencyDefinitionSources, ImportedDependencySelectionPlan,
    ImportedDependencySelectionPlanBuildError,
};
use crate::semantic_world::{ImportedProvider, ImportedSemanticWorld};
use crate::{
    CallableInterfaceRecordV1, CallableSourceInterfaceV1, CanonicalNominalInterfacesV1,
    ExportConstValueV1, ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1,
    ParamFreeNominalCallableV1, PropertyInterfaceRecordV1, TypeAliasInterfaceRecordV1,
};

#[derive(Clone, Debug)]
pub(super) struct CallableCatalogEntry {
    pub(super) provider: ConeIdentity,
    pub(super) name: super::intrinsics::CallableCatalogName,
    pub(super) interface: CallableInterfaceRecordV1,
    pub(super) source: Option<CallableSourceInterfaceV1>,
    pub(super) capability: Option<ParamFreeNominalCallableV1>,
    pub(super) initialization_unit: Option<scoop_identity::PersistentInitializationUnitId>,
    pub(super) default_templates: BTreeMap<ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1>,
    pub(super) definition_sources: Arc<ImportedDependencyDefinitionSources>,
}

#[derive(Clone, Debug)]
pub(super) struct ConstantCatalogEntry {
    pub(super) provider: ConeIdentity,
    pub(super) record: ExportConstValueV1,
    pub(super) exact_type: Option<scoop_identity::PersistentExactTypeId>,
    pub(super) definition_sources: Arc<ImportedDependencyDefinitionSources>,
}

#[derive(Clone, Debug)]
pub(super) struct PropertyCatalogEntry {
    pub(super) provider: ConeIdentity,
    pub(super) name: scoop_identity::CanonicalIdentifier,
    pub(super) interface: PropertyInterfaceRecordV1,
}

#[derive(Clone, Debug)]
pub(super) struct TypeAliasCatalogEntry {
    pub(super) provider: ConeIdentity,
    pub(super) interface: TypeAliasInterfaceRecordV1,
    pub(super) expansion: scoop_identity::SignatureTypeKey,
}

#[derive(Debug)]
pub(super) struct DependencyCatalog {
    pub(super) nominals:
        BTreeMap<scoop_identity::PersistentTypeId, Arc<super::ImportedNominalDeclaration>>,
    pub(super) consumer: ConeIdentity,
    pub(super) callables: BTreeMap<CallableTemplateOrigin, CallableCatalogEntry>,
    pub(super) properties: BTreeMap<PropertyOwner, PropertyCatalogEntry>,
    pub(super) constants: BTreeMap<PersistentPropertyId, ConstantCatalogEntry>,
    pub(super) type_aliases: BTreeMap<PersistentTypeAliasId, TypeAliasCatalogEntry>,
}

impl ImportedSemanticWorld<'_> {
    pub fn dependency_selection_plan(
        &self,
    ) -> Result<ImportedDependencySelectionPlan, ImportedDependencySelectionPlanBuildError> {
        let classifier = self
            .nominal_exact_leaf_classifier(&CanonicalNominalInterfacesV1::default())
            .map_err(ImportedDependencySelectionPlanBuildError::NominalClassifier)?;
        let mut callables = BTreeMap::new();
        let mut properties = BTreeMap::new();
        let mut constants = BTreeMap::new();
        let mut type_aliases = BTreeMap::new();
        let mut nominals = BTreeMap::new();
        for provider in &self.providers {
            for declaration in super::nominals::declarations(provider)? {
                let id = declaration.identity.id();
                if nominals.insert(id, declaration).is_some() {
                    return Err(ImportedDependencySelectionPlanBuildError::DuplicateNominal(
                        id,
                    ));
                }
            }
            let definition_sources = Arc::new(imported_definition_sources(provider)?);
            for callable in provider.interface().callable_interfaces().records() {
                let declaration = callable.declaration();
                let initialization_unit = match declaration {
                    CallableTemplateOrigin::Accessor(accessor) => {
                        crate::initialization_dependencies::accessor_initialization_unit(
                            accessor,
                            provider.interface().property_interfaces(),
                            provider
                                .foundation()
                                .canonical_for_semantic_authority()
                                .type_source_initialization_records(),
                        )
                        .map_err(ImportedDependencySelectionPlanBuildError::Initialization)?
                    }
                    _ => None,
                };
                let entry = CallableCatalogEntry {
                    name: super::intrinsics::callable_catalog_name(provider, declaration)?,
                    provider: provider.identity(),
                    interface: callable.clone(),
                    source: provider
                        .interface()
                        .source_interfaces()
                        .get(declaration)
                        .cloned(),
                    capability: classifier
                        .classify_callable(callable)
                        .map_err(ImportedDependencySelectionPlanBuildError::Classification)?,
                    initialization_unit,
                    default_templates: provider
                        .interface()
                        .default_templates()
                        .records()
                        .iter()
                        .filter(|template| template.key().owner() == declaration)
                        .map(|template| (template.key(), template.clone()))
                        .collect(),
                    definition_sources: Arc::clone(&definition_sources),
                };
                if callables.insert(declaration, entry).is_some() {
                    return Err(
                        ImportedDependencySelectionPlanBuildError::DuplicateCallable(declaration),
                    );
                }
            }
            for property in provider.interface().property_interfaces().records() {
                let declaration = property.declaration();
                let entry = PropertyCatalogEntry {
                    provider: provider.identity(),
                    name: super::intrinsics::property_catalog_name(provider, declaration)?,
                    interface: property.clone(),
                };
                if properties.insert(declaration, entry).is_some() {
                    return Err(
                        ImportedDependencySelectionPlanBuildError::DuplicateProperty(declaration),
                    );
                }
            }
            for constant in provider.interface().constants().records() {
                let property = constant.property();
                let entry = ConstantCatalogEntry {
                    provider: provider.identity(),
                    record: constant.clone(),
                    exact_type: classifier
                        .classify(constant.value_type())
                        .map_err(ImportedDependencySelectionPlanBuildError::Classification)?,
                    definition_sources: Arc::clone(&definition_sources),
                };
                if constants.insert(property, entry).is_some() {
                    return Err(
                        ImportedDependencySelectionPlanBuildError::DuplicateConstant(property),
                    );
                }
            }
            for alias in provider.interface().type_aliases().records() {
                let declaration = alias.alias();
                let expansion = provider.alias_expansions().get(declaration).ok_or(
                    ImportedDependencySelectionPlanBuildError::MissingTypeAliasExpansion(
                        declaration,
                    ),
                )?;
                let entry = TypeAliasCatalogEntry {
                    provider: provider.identity(),
                    interface: alias.clone(),
                    expansion: expansion.target().clone(),
                };
                if type_aliases.insert(declaration, entry).is_some() {
                    return Err(
                        ImportedDependencySelectionPlanBuildError::DuplicateTypeAlias(declaration),
                    );
                }
            }
        }
        Ok(ImportedDependencySelectionPlan {
            catalog: Arc::new(DependencyCatalog {
                nominals,
                consumer: self.current,
                callables,
                properties,
                constants,
                type_aliases,
            }),
            callables: BTreeMap::new(),
            constants: BTreeMap::new(),
            type_aliases: BTreeMap::new(),
        })
    }
}

fn imported_definition_sources(
    provider: &ImportedProvider<'_>,
) -> Result<ImportedDependencyDefinitionSources, ImportedDependencySelectionPlanBuildError> {
    let mut records = BTreeMap::new();
    let mut contexts = BTreeMap::new();
    for source in provider.interface().definition_sources().sources() {
        let origin = source.origin();
        let record = provider
            .foundation()
            .source_record(origin.source())
            .ok_or_else(
                || ImportedDependencySelectionPlanBuildError::MissingDefinitionSource {
                    provider: provider.identity(),
                    source: origin.source().clone(),
                },
            )?;
        records
            .entry(origin.source().clone())
            .or_insert_with(|| record.clone());
        let context = provider
            .foundation()
            .source_context_key(origin.context())
            .ok_or(
                ImportedDependencySelectionPlanBuildError::MissingDefinitionContext {
                    provider: provider.identity(),
                    context: origin.context(),
                },
            )?;
        contexts
            .entry(origin.context())
            .or_insert_with(|| context.clone());
    }
    Ok(ImportedDependencyDefinitionSources { records, contexts })
}
