use std::collections::BTreeMap;
use std::sync::Arc;

use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, DefinitionOriginSubject, PersistentPropertyId,
    PersistentTypeAliasId, PropertyOwner,
};

use super::{
    ImportedDependencyDefinitionSources, ImportedDependencySelectionPlan,
    ImportedDependencySelectionPlanBuildError,
};
use crate::semantic_world::{ImportedProvider, ImportedSemanticWorld};
use crate::{
    CallableDeclarationRecordV1, CallableSourceInterfaceV1, CanonicalNominalInterfacesV1,
    ExportConstValueV1, ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1,
    ParamFreeNominalCallableV1, PropertyDeclarationRecordV1, TypeAliasInterfaceRecordV1,
};

#[derive(Clone, Debug)]
pub(super) struct CallableCatalogEntry {
    pub(super) provider: ConeIdentity,
    pub(super) name: super::intrinsics::CallableCatalogName,
    pub(super) interface: CallableDeclarationRecordV1,
    pub(super) source: Option<CallableSourceInterfaceV1>,
    pub(super) capability: Option<ParamFreeNominalCallableV1>,
    pub(super) initialization_unit: Option<scoop_identity::PersistentInitializationUnitId>,
    pub(super) default_templates: BTreeMap<ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1>,
    pub(super) definition_sources: Arc<ImportedDependencyDefinitionSources>,
    pub(super) definition_origin: crate::ExportDefinitionSourceV1,
    pub(super) callable_body: Option<Arc<crate::ExportGenericCallableBodyV1>>,
    pub(super) native_contract: Option<Arc<scoop_identity::SourceNativeExternalContractRecord>>,
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
    pub(super) interface: PropertyDeclarationRecordV1,
    pub(super) definition_origin: crate::ExportDefinitionSourceV1,
    pub(super) native_contract: Option<Arc<scoop_identity::SourceNativeExternalContractRecord>>,
}

#[derive(Clone, Debug)]
pub(super) struct TypeAliasCatalogEntry {
    pub(super) provider: ConeIdentity,
    pub(super) interface: TypeAliasInterfaceRecordV1,
    pub(super) expansion: scoop_identity::SignatureTypeKey,
}

#[derive(Debug)]
pub(super) struct GeneratedCallableCatalogEntry {
    pub(super) definition: crate::concrete::GeneratedCallableRecord,
    pub(super) origin: Option<crate::ExportDefinitionSourceV1>,
}

#[derive(Debug)]
pub(super) struct DependencyCatalog {
    pub(super) static_namespaces:
        BTreeMap<crate::SourceNominalId, Vec<crate::DirectNamedPublicBindingGroup>>,
    pub(super) nominals: BTreeMap<crate::SourceNominalId, Arc<super::ImportedNominalDeclaration>>,
    pub(super) nominal_visibilities: BTreeMap<crate::SourceNominalId, crate::DeclaredVisibilityV1>,
    pub(super) consumer: ConeIdentity,
    pub(super) callables: BTreeMap<CallableTemplateOrigin, CallableCatalogEntry>,
    pub(super) bodies: BTreeMap<crate::DefaultCallableDeclarationV1, super::ImportedCallableBody>,
    pub(super) generated_callables:
        BTreeMap<scoop_identity::PersistentGeneratedCallableId, GeneratedCallableCatalogEntry>,
    pub(super) callback_registrations: BTreeMap<
        scoop_identity::PersistentCallbackRegistrationId,
        (
            crate::HirCallbackRegistrationIdentity,
            scoop_identity::DefinitionOrigin,
        ),
    >,
    pub(super) initializations:
        BTreeMap<scoop_identity::PersistentGenericTypeId, super::ImportedNominalInitialization>,
    pub(super) delegates: BTreeMap<
        scoop_identity::PersistentExtensionPropertyId,
        Arc<crate::ExportGenericDelegateTemplateV1>,
    >,
    pub(super) properties: BTreeMap<PropertyOwner, PropertyCatalogEntry>,
    pub(super) constants: BTreeMap<PersistentPropertyId, ConstantCatalogEntry>,
    pub(super) type_aliases: BTreeMap<PersistentTypeAliasId, TypeAliasCatalogEntry>,
    pub(super) annotations:
        BTreeMap<scoop_identity::PersistentAnnotationId, super::ImportedAnnotationDeclaration>,
    pub(super) annotated_targets:
        BTreeMap<crate::AnnotationTargetV1, Vec<crate::AnnotationApplicationV1>>,
}

impl ImportedSemanticWorld<'_> {
    pub fn dependency_selection_plan(
        &self,
    ) -> Result<ImportedDependencySelectionPlan, ImportedDependencySelectionPlanBuildError> {
        let classifier = self
            .nominal_exact_leaf_classifier(&CanonicalNominalInterfacesV1::default())
            .map_err(ImportedDependencySelectionPlanBuildError::NominalClassifier)?;
        let mut callables = BTreeMap::new();
        let mut bodies = BTreeMap::new();
        let mut generated_callables = BTreeMap::new();
        let mut callback_registrations = BTreeMap::new();
        let mut initializations = BTreeMap::new();
        let mut delegates = BTreeMap::new();
        let mut properties = BTreeMap::new();
        let mut constants = BTreeMap::new();
        let mut type_aliases = BTreeMap::new();
        let mut annotations = BTreeMap::new();
        let mut annotated_targets = BTreeMap::new();
        let mut nominals = BTreeMap::new();
        let mut nominal_visibilities = BTreeMap::new();
        let mut static_namespaces = BTreeMap::new();
        for provider in &self.providers {
            for declaration in provider.interface().annotations().declarations() {
                let (_, key) = provider
                    .foundation()
                    .canonical_for_semantic_authority()
                    .annotation_by_bytes(declaration.annotation.as_array())
                    .expect("indexed annotation has its source key");
                annotations.insert(
                    declaration.annotation,
                    super::ImportedAnnotationDeclaration {
                        source: key.clone(),
                        declaration: declaration.clone(),
                    },
                );
            }
            for target in provider.interface().annotations().targets() {
                annotated_targets.insert(target.target, target.annotations.clone());
            }
            let foundation = provider.foundation().canonical_for_semantic_authority();
            for record in foundation.callback_registration_records() {
                callback_registrations
                    .entry(record.id())
                    .or_insert_with(|| {
                        let origin = foundation
                            .definition_origin(DefinitionOriginSubject::CallbackRegistration(
                                record.id(),
                            ))
                            .expect("a validated callback registration has its definition origin");
                        (record.clone(), origin.origin().clone())
                    });
            }
            for record in provider
                .foundation()
                .canonical_for_semantic_authority()
                .type_source_generated_callable_records()
            {
                let origin = provider
                    .foundation()
                    .canonical_for_semantic_authority()
                    .definition_origin(DefinitionOriginSubject::GeneratedCallable(record.id()))
                    .map(|record| crate::ExportDefinitionSourceV1::new(record.origin().clone()));
                generated_callables
                    .entry(record.id())
                    .and_modify(|entry: &mut GeneratedCallableCatalogEntry| {
                        if entry.origin.is_none() {
                            entry.origin = origin.clone();
                        }
                    })
                    .or_insert_with(|| GeneratedCallableCatalogEntry {
                        definition: record.clone(),
                        origin,
                    });
            }
            for nominal in provider.interface().nominal_interfaces().all_records() {
                nominal_visibilities.insert(
                    nominal.declaration(),
                    nominal.declaration_details().declared_visibility(),
                );
            }
            for nominal in provider.interface().nominal_interfaces().records() {
                let owner = nominal.declaration();
                let namespace = self
                    .imported_static_namespace(owner)
                    .expect("the dependency world indexes every public static namespace");
                static_namespaces.insert(owner, namespace.snapshot());
            }
            for declaration in super::nominals::declarations(provider)? {
                let id = declaration.owner();
                if nominals.insert(id, declaration).is_some() {
                    return Err(ImportedDependencySelectionPlanBuildError::DuplicateNominal(
                        id,
                    ));
                }
            }
            let definition_sources = Arc::new(imported_definition_sources(provider, self)?);
            for initialization in provider.interface().generic_initializations().records() {
                initializations.insert(
                    initialization.owner(),
                    super::ImportedNominalInitialization {
                        initialization: Arc::new(initialization.clone()),
                        definition_sources: Arc::clone(&definition_sources),
                    },
                );
            }
            for delegate in provider.interface().generic_delegates().records() {
                if delegates
                    .insert(delegate.property(), Arc::new(delegate.clone()))
                    .is_some()
                {
                    return Err(
                        ImportedDependencySelectionPlanBuildError::DuplicateGenericDelegate(
                            delegate.property(),
                        ),
                    );
                }
            }
            for body in provider
                .interface()
                .generic_callable_bodies()
                .records()
                .iter()
                .chain(
                    provider
                        .interface()
                        .generic_delegates()
                        .records()
                        .iter()
                        .map(crate::ExportGenericDelegateTemplateV1::initializer),
                )
            {
                let declaration = match body.owner() {
                    crate::DefaultCallableDeclarationV1::Function(id) => {
                        Some(CallableTemplateOrigin::Function(id))
                    }
                    crate::DefaultCallableDeclarationV1::GenericFunction(id) => {
                        Some(CallableTemplateOrigin::GenericFunction(id))
                    }
                    crate::DefaultCallableDeclarationV1::PropertyAccessor(_)
                    | crate::DefaultCallableDeclarationV1::Generated(_) => None,
                };
                let source_declaration = declaration
                    .map(|declaration| {
                        let foundation = provider.foundation().canonical_for_semantic_authority();
                        let key = match declaration {
                            CallableTemplateOrigin::Function(id) => foundation
                                .function_by_bytes(id.as_array())
                                .map(|(_, key)| key),
                            CallableTemplateOrigin::GenericFunction(id) => foundation
                                .generic_function_by_bytes(id.as_array())
                                .map(|(_, key)| key),
                            _ => unreachable!("named bodies retain function declarations"),
                        };
                        key.filter(|key| {
                            matches!(key.name(), scoop_identity::DeclarationName::Named(_))
                        })
                        .cloned()
                        .map(Arc::new)
                        .ok_or(
                            ImportedDependencySelectionPlanBuildError::MissingCallableSourceName(
                                declaration,
                            ),
                        )
                    })
                    .transpose()?;
                let source = super::ImportedCallableBody {
                    body: Arc::new(body.clone()),
                    definition_sources: Arc::clone(&definition_sources),
                    source_declaration,
                };
                if bodies.insert(body.owner(), source).is_some() {
                    return Err(
                        ImportedDependencySelectionPlanBuildError::DuplicateCallableBody(
                            body.owner(),
                        ),
                    );
                }
            }
            for callable in provider
                .interface()
                .callable_interfaces()
                .all_declarations()
            {
                let declaration = callable.declaration();
                let subject = match declaration {
                    CallableTemplateOrigin::Function(id) => DefinitionOriginSubject::Function(id),
                    CallableTemplateOrigin::GenericFunction(id) => {
                        DefinitionOriginSubject::GenericFunction(id)
                    }
                    CallableTemplateOrigin::Accessor(id) => {
                        DefinitionOriginSubject::PropertyAccessor(id)
                    }
                    CallableTemplateOrigin::Constructor(id) => {
                        DefinitionOriginSubject::Constructor(id)
                    }
                    CallableTemplateOrigin::VariantConstructor(id) => {
                        DefinitionOriginSubject::EnumVariant(id)
                    }
                };
                let definition_origin = provider
                    .foundation()
                    .canonical_for_semantic_authority()
                    .definition_origin(subject)
                    .ok_or(
                        ImportedDependencySelectionPlanBuildError::MissingDefinitionOrigin(subject),
                    )?;
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
                    native_contract: provider.foundation().canonical_for_semantic_authority().source_native_contracts().iter()
                        .find(|record| matches!((declaration, record.key().owner()),
                            (CallableTemplateOrigin::Function(function), scoop_identity::SourceNativeExternalOwner::Function(owner)) if function == owner))
                        .cloned().map(Arc::new),
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
                    definition_origin: crate::ExportDefinitionSourceV1::new(
                        definition_origin.origin().clone(),
                    ),
                    callable_body: match declaration {
                        CallableTemplateOrigin::Function(id) => {
                            Some(crate::DefaultCallableDeclarationV1::Function(id))
                        }
                        CallableTemplateOrigin::GenericFunction(id) => {
                            Some(crate::DefaultCallableDeclarationV1::GenericFunction(id))
                        }
                        CallableTemplateOrigin::Accessor(id) => {
                            Some(crate::DefaultCallableDeclarationV1::PropertyAccessor(id))
                        }
                        CallableTemplateOrigin::Constructor(_)
                        | CallableTemplateOrigin::VariantConstructor(_) => None,
                    }
                    .and_then(|owner| bodies.get(&owner))
                    .map(|source| Arc::clone(&source.body)),
                };
                if callables.insert(declaration, entry).is_some() {
                    return Err(
                        ImportedDependencySelectionPlanBuildError::DuplicateCallable(declaration),
                    );
                }
            }
            for property in provider
                .interface()
                .property_interfaces()
                .all_declarations()
            {
                let declaration = property.declaration();
                let subject = match declaration {
                    PropertyOwner::Property(id) => DefinitionOriginSubject::Property(id),
                    PropertyOwner::ExtensionProperty(id) => {
                        DefinitionOriginSubject::ExtensionProperty(id)
                    }
                };
                let origin = provider
                    .foundation()
                    .canonical_for_semantic_authority()
                    .definition_origin(subject)
                    .ok_or(
                        ImportedDependencySelectionPlanBuildError::MissingDefinitionOrigin(subject),
                    )?;
                let entry = PropertyCatalogEntry {
                    provider: provider.identity(),
                    name: super::intrinsics::property_catalog_name(provider, declaration)?,
                    interface: property.clone(),
                    definition_origin: crate::ExportDefinitionSourceV1::new(origin.origin().clone()),
                    native_contract: provider.foundation().canonical_for_semantic_authority().source_native_contracts().iter()
                        .find(|record| matches!((declaration, record.key().owner()),
                            (PropertyOwner::Property(property), scoop_identity::SourceNativeExternalOwner::Property(owner)) if property == owner))
                        .cloned().map(Arc::new),
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
                static_namespaces,
                nominals,
                nominal_visibilities,
                consumer: self.current,
                callables,
                bodies,
                generated_callables,
                callback_registrations,
                initializations,
                delegates,
                properties,
                constants,
                type_aliases,
                annotations,
                annotated_targets,
            }),
            callables: BTreeMap::new(),
            constants: BTreeMap::new(),
            type_aliases: BTreeMap::new(),
        })
    }
}

fn imported_definition_sources(
    provider: &ImportedProvider<'_>,
    world: &ImportedSemanticWorld<'_>,
) -> Result<ImportedDependencyDefinitionSources, ImportedDependencySelectionPlanBuildError> {
    let foundation = provider.foundation().canonical_for_semantic_authority();
    let records = foundation
        .source_records()
        .iter()
        .map(|record| (record.identity().clone(), record.clone()))
        .collect();
    let contexts = foundation
        .source_context_records()
        .map(|(id, key)| {
            let origin = world.positions.get(&key.source().cone()).map(|index| {
                world.providers[*index]
                    .foundation()
                    .canonical_for_semantic_authority()
            });
            let names = origin
                .and_then(|origin| origin.source_context_names(key))
                .ok_or(
                    ImportedDependencySelectionPlanBuildError::MissingDefinitionContext {
                        provider: key.source().cone(),
                        context: id,
                    },
                )?;
            Ok((id, (key.clone(), names)))
        })
        .collect::<Result<_, _>>()?;
    Ok(ImportedDependencyDefinitionSources { records, contexts })
}

impl ImportedDependencySelectionPlan {
    /// Resolves a name within the actual declaration's static namespace.
    pub fn static_bindings(
        &self,
        owner: crate::SourceNominalId,
        namespace: scoop_identity::BindingNamespace,
        name: &str,
    ) -> &[crate::DirectImportedTargetBinding] {
        self.catalog
            .static_namespaces
            .get(&owner)
            .and_then(|groups| {
                groups
                    .iter()
                    .find(|group| group.namespace() == namespace && group.name().as_str() == name)
            })
            .map_or(&[], |group| group.targets())
    }
}
