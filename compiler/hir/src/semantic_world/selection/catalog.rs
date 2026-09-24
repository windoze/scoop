use std::collections::BTreeMap;
use std::sync::Arc;

use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, PersistentPropertyId, PersistentTypeAliasId,
    PropertyOwner,
};

use super::{
    DependencyProjectionId, DependencySelectionId, ImportedDependencyCallableId,
    ImportedDependencyConstantId, ImportedDependencyDefinitionSources,
    ImportedDependencySelectionPlan, ImportedDependencySelectionPlanBuildError,
    ImportedDependencyTypeAliasId, NEXT_PROJECTION, NEXT_SELECTION, next_id,
};
use crate::semantic_world::{DirectImportedTargetBinding, ImportedProvider, ImportedSemanticWorld};
use crate::{
    CallableInterfaceRecordV1, CallableSourceInterfaceV1, CanonicalNominalInterfacesV1,
    ExportConstValueV1, ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1,
    ImportedProviderCertificate, ImportedTarget, ParamFreeNominalCallableV1,
    PropertyInterfaceRecordV1, TypeAliasInterfaceRecordV1,
};

#[derive(Clone, Debug)]
pub(super) struct CallableCatalogEntry {
    pub(super) certificate: ImportedProviderCertificate,
    pub(super) name: super::intrinsics::CallableCatalogName,
    pub(super) interface: CallableInterfaceRecordV1,
    pub(super) source: Option<CallableSourceInterfaceV1>,
    pub(super) capability: Option<ParamFreeNominalCallableV1>,
    pub(super) default_templates: BTreeMap<ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1>,
    pub(super) definition_sources: Arc<ImportedDependencyDefinitionSources>,
}

#[derive(Clone, Debug)]
pub(super) struct ConstantCatalogEntry {
    pub(super) certificate: ImportedProviderCertificate,
    pub(super) record: ExportConstValueV1,
    pub(super) exact_type: Option<scoop_identity::PersistentExactTypeId>,
    pub(super) definition_sources: Arc<ImportedDependencyDefinitionSources>,
}

#[derive(Clone, Debug)]
pub(super) struct PropertyCatalogEntry {
    pub(super) certificate: ImportedProviderCertificate,
    pub(super) interface: PropertyInterfaceRecordV1,
}

#[derive(Clone, Debug)]
pub(super) struct TypeAliasCatalogEntry {
    pub(super) certificate: ImportedProviderCertificate,
    pub(super) interface: TypeAliasInterfaceRecordV1,
    pub(super) expansion: scoop_identity::SignatureTypeKey,
}

#[derive(Debug)]
pub(super) struct DependencyCatalog {
    pub(super) direct_binding_witnesses:
        Arc<BTreeMap<crate::ExternalHirTargetV1, Vec<crate::DependencyBindingWitnessV1>>>,
    pub(super) world_brand: u64,
    pub(super) consumer: ConeIdentity,
    pub(super) projection: DependencyProjectionId,
    pub(super) callables: BTreeMap<CallableTemplateOrigin, CallableCatalogEntry>,
    pub(super) callable_ids: BTreeMap<CallableTemplateOrigin, ImportedDependencyCallableId>,
    pub(super) properties: BTreeMap<PropertyOwner, PropertyCatalogEntry>,
    pub(super) constants: BTreeMap<PersistentPropertyId, ConstantCatalogEntry>,
    pub(super) constant_ids: BTreeMap<PersistentPropertyId, ImportedDependencyConstantId>,
    pub(super) type_aliases: BTreeMap<PersistentTypeAliasId, TypeAliasCatalogEntry>,
    pub(super) type_alias_ids: BTreeMap<PersistentTypeAliasId, ImportedDependencyTypeAliasId>,
    pub(super) direct_callable_bindings:
        BTreeMap<CallableTemplateOrigin, DirectImportedTargetBinding>,
}

impl ImportedSemanticWorld<'_> {
    pub fn dependency_selection_plan(
        &self,
    ) -> Result<ImportedDependencySelectionPlan, ImportedDependencySelectionPlanBuildError> {
        let classifier = self
            .nominal_exact_leaf_classifier(&CanonicalNominalInterfacesV1::default())
            .map_err(ImportedDependencySelectionPlanBuildError::NominalClassifier)?;
        let projection = DependencyProjectionId(next_id(&NEXT_PROJECTION, "dependency projection"));
        let mut callables = BTreeMap::new();
        let mut properties = BTreeMap::new();
        let mut constants = BTreeMap::new();
        let mut type_aliases = BTreeMap::new();
        for provider in &self.providers {
            let definition_sources = Arc::new(imported_definition_sources(provider)?);
            for callable in provider.interface().callable_interfaces().records() {
                let declaration = callable.declaration();
                let entry = CallableCatalogEntry {
                    name: super::intrinsics::callable_catalog_name(provider, declaration)?,
                    certificate: provider.certificate().clone(),
                    interface: callable.clone(),
                    source: provider
                        .interface()
                        .source_interfaces()
                        .get(declaration)
                        .cloned(),
                    capability: classifier
                        .classify_callable(callable)
                        .map_err(ImportedDependencySelectionPlanBuildError::Classification)?,
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
                    certificate: provider.certificate().clone(),
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
                    certificate: provider.certificate().clone(),
                    record: constant.clone(),
                    exact_type: classifier.classify(constant.value_type()),
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
                    certificate: provider.certificate().clone(),
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
        let callable_ids = callables
            .keys()
            .copied()
            .enumerate()
            .map(|(index, declaration)| {
                u32::try_from(index)
                    .map(|index| (declaration, ImportedDependencyCallableId(index)))
                    .map_err(
                        |_| ImportedDependencySelectionPlanBuildError::TooManyCallables {
                            count: callables.len(),
                        },
                    )
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let constant_ids = constants
            .keys()
            .copied()
            .enumerate()
            .map(|(index, property)| {
                u32::try_from(index)
                    .map(|index| (property, ImportedDependencyConstantId(index)))
                    .map_err(
                        |_| ImportedDependencySelectionPlanBuildError::TooManyConstants {
                            count: constants.len(),
                        },
                    )
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let type_alias_ids = type_aliases
            .keys()
            .copied()
            .enumerate()
            .map(|(index, alias)| {
                u32::try_from(index)
                    .map(|index| (alias, ImportedDependencyTypeAliasId(index)))
                    .map_err(
                        |_| ImportedDependencySelectionPlanBuildError::TooManyTypeAliases {
                            count: type_aliases.len(),
                        },
                    )
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let direct_callable_bindings = self.direct_callable_bindings()?;
        Ok(ImportedDependencySelectionPlan {
            catalog: Arc::new(DependencyCatalog {
                direct_binding_witnesses: Arc::new(self.direct_binding_witnesses()),
                world_brand: self.brand,
                consumer: self.current,
                projection,
                callables,
                callable_ids,
                properties,
                constants,
                constant_ids,
                type_aliases,
                type_alias_ids,
                direct_callable_bindings,
            }),
            selection: DependencySelectionId(next_id(&NEXT_SELECTION, "dependency selection")),
            callables: BTreeMap::new(),
            constants: BTreeMap::new(),
            type_aliases: BTreeMap::new(),
        })
    }

    fn direct_callable_bindings(
        &self,
    ) -> Result<
        BTreeMap<CallableTemplateOrigin, DirectImportedTargetBinding>,
        ImportedDependencySelectionPlanBuildError,
    > {
        let mut bindings = BTreeMap::new();
        for provider in self
            .direct
            .iter()
            .map(|provider| &self.providers[provider.index()])
        {
            for binding in provider.public_bindings() {
                let declarations = direct_binding_callable_declarations(provider, binding.target());
                for declaration in declarations {
                    let candidate = DirectImportedTargetBinding::new(
                        binding.key().binding_target(),
                        binding.target(),
                        binding.conflict_key().clone(),
                        binding.lookup_sources().to_vec(),
                    );
                    insert_direct_binding(&mut bindings, declaration, candidate)?;
                }
            }
        }
        Ok(bindings)
    }
}

fn direct_binding_callable_declarations(
    provider: &ImportedProvider<'_>,
    target: ImportedTarget,
) -> Vec<CallableTemplateOrigin> {
    match target {
        ImportedTarget::Function(id) => vec![CallableTemplateOrigin::Function(id.persistent())],
        ImportedTarget::GenericFunction(id) => {
            vec![CallableTemplateOrigin::GenericFunction(id.persistent())]
        }
        ImportedTarget::Property(id) => {
            property_accessors(provider, PropertyOwner::Property(id.persistent()))
        }
        ImportedTarget::ExtensionProperty(id) => {
            property_accessors(provider, PropertyOwner::ExtensionProperty(id.persistent()))
        }
        ImportedTarget::Type(_)
        | ImportedTarget::GenericType(_)
        | ImportedTarget::ObjectValue(_)
        | ImportedTarget::TypeAlias(_)
        | ImportedTarget::EnumVariant(_) => Vec::new(),
    }
}

fn property_accessors(
    provider: &ImportedProvider<'_>,
    property: PropertyOwner,
) -> Vec<CallableTemplateOrigin> {
    let Some(property) = provider.interface().property_interfaces().get(property) else {
        return Vec::new();
    };
    let capability = property.capability();
    let mut declarations = vec![CallableTemplateOrigin::Accessor(capability.getter())];
    declarations.extend(capability.setter().map(CallableTemplateOrigin::Accessor));
    declarations
}

fn insert_direct_binding(
    bindings: &mut BTreeMap<CallableTemplateOrigin, DirectImportedTargetBinding>,
    declaration: CallableTemplateOrigin,
    candidate: DirectImportedTargetBinding,
) -> Result<(), ImportedDependencySelectionPlanBuildError> {
    match bindings.entry(declaration) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(candidate);
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            entry.get_mut().try_merge(candidate).map_err(|source| {
                ImportedDependencySelectionPlanBuildError::DirectBindingMerge {
                    declaration,
                    source,
                }
            })?
        }
    }
    Ok(())
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
