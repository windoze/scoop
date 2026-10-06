//! Persistent identities for compiler-managed initialization units.

use std::collections::HashSet;
use std::ops::Index;

use la_arena::{Arena, Idx};
use scoop_identity::{CborIdentityRecord, InitializationUnitKey, PersistentInitializationUnitId};

use crate::{
    CompanionRelation, DelegateStorage, DelegateStorageLocation, Function, Global, GlobalStorage,
    HirNominalIdentities, HirPropertyIdentities, HirPropertyIdentity, HirStaticInitialState,
    InitializationFailureRoot, InitializationSchedule, InitializationUnit, InitializationUnitId,
    InitializationUnitKind, ObjectDecl, ObjectKind, Property, PropertyBacking, PropertyOwner,
    PropertyRepresentation, SingletonPublishedRoot, SingletonValue, TopLevelInitialization,
};

pub type HirInitializationUnitIdentity =
    CborIdentityRecord<PersistentInitializationUnitId, InitializationUnitKey>;

mod error;
pub use error::{HirInitializationUnitIdentityError, InitializationRelation};

/// Total persistent identity relation for the initialization-unit arena.
#[derive(Clone, Debug)]
pub struct HirInitializationUnitIdentities {
    identities: Vec<HirInitializationUnitIdentity>,
}

impl HirInitializationUnitIdentities {
    pub fn records(&self) -> &[HirInitializationUnitIdentity] {
        &self.identities
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_declarations(
        units: &Arena<InitializationUnit>,
        failure_roots: &Arena<InitializationFailureRoot>,
        functions: &Arena<Function>,
        globals: &Arena<Global>,
        objects: &Arena<ObjectDecl>,
        companions: &Arena<CompanionRelation>,
        singleton_values: &Arena<SingletonValue>,
        published_roots: &Arena<SingletonPublishedRoot>,
        properties: &Arena<Property>,
        delegate_storages: &Arena<DelegateStorage>,
        generic_delegates: &Arena<crate::GenericDelegateTemplate>,
        nominal_identities: &HirNominalIdentities,
        property_identities: &HirPropertyIdentities,
    ) -> Result<Self, HirInitializationUnitIdentityError> {
        let mut identities = Vec::with_capacity(units.len());
        let mut persistent_ids = HashSet::new();
        let mut seen_failure_roots = vec![false; failure_roots.len()];
        let mut seen_functions = HashSet::new();
        let mut seen_values = vec![false; singleton_values.len()];
        let mut seen_published_roots = vec![false; published_roots.len()];

        for (unit_id, unit) in units.iter() {
            validate_common_relations(
                unit_id,
                unit,
                failure_roots,
                functions,
                &mut seen_failure_roots,
                &mut seen_functions,
            )?;
            let key = match unit.kind {
                InitializationUnitKind::GenericDelegatedExtension { property, template } => {
                    if unit.schedule != InitializationSchedule::LazyAccess {
                        return Err(HirInitializationUnitIdentityError::Schedule {
                            unit: raw_index(unit_id),
                        });
                    }
                    if raw_index(template) as usize >= generic_delegates.len()
                        || raw_index(property) as usize >= properties.len()
                    {
                        return Err(HirInitializationUnitIdentityError::StorageRelation {
                            unit: raw_index(unit_id),
                        });
                    }
                    let delegate = &generic_delegates[template];
                    if delegate.property != property
                        || delegate.initialization != unit_id
                        || !matches!(properties[delegate.property].representation,
                            PropertyRepresentation::GenericDelegated { template: owner } if owner == template)
                    {
                        return Err(HirInitializationUnitIdentityError::StorageRelation {
                            unit: raw_index(unit_id),
                        });
                    }
                    let HirPropertyIdentity::Extension(property) =
                        &property_identities[delegate.property]
                    else {
                        return Err(HirInitializationUnitIdentityError::PropertyKind {
                            unit: raw_index(unit_id),
                        });
                    };
                    InitializationUnitKey::ExtensionProperty(property.id())
                }
                InitializationUnitKind::EagerTopLevel { property, storage } => {
                    if unit.schedule != InitializationSchedule::EagerStartup {
                        return Err(HirInitializationUnitIdentityError::Schedule {
                            unit: raw_index(unit_id),
                        });
                    }
                    eager_property_key(
                        unit_id,
                        property,
                        storage,
                        globals,
                        properties,
                        delegate_storages,
                        property_identities,
                    )?
                }
                InitializationUnitKind::LazySingleton {
                    value,
                    published_root,
                } => {
                    if unit.schedule != InitializationSchedule::LazyAccess {
                        return Err(HirInitializationUnitIdentityError::Schedule {
                            unit: raw_index(unit_id),
                        });
                    }
                    singleton_key(
                        unit_id,
                        value,
                        published_root,
                        objects,
                        companions,
                        singleton_values,
                        published_roots,
                        nominal_identities,
                        &mut seen_values,
                        &mut seen_published_roots,
                    )?
                }
            };

            let record = CborIdentityRecord::from_key(key).map_err(|error| {
                HirInitializationUnitIdentityError::InvalidIdentity {
                    unit: raw_index(unit_id),
                    error,
                }
            })?;
            if !persistent_ids.insert(record.id()) {
                return Err(
                    HirInitializationUnitIdentityError::DuplicatePersistentIdentity {
                        unit: raw_index(unit_id),
                    },
                );
            }
            identities.push(record);
        }

        require_coverage(InitializationRelation::FailureRoot, &seen_failure_roots)?;
        require_coverage(InitializationRelation::SingletonValue, &seen_values)?;
        require_coverage(
            InitializationRelation::SingletonPublishedRoot,
            &seen_published_roots,
        )?;
        Ok(Self { identities })
    }
}

impl Index<InitializationUnitId> for HirInitializationUnitIdentities {
    type Output = HirInitializationUnitIdentity;

    fn index(&self, id: InitializationUnitId) -> &Self::Output {
        &self.identities[local_index(id)]
    }
}

fn validate_common_relations(
    unit_id: InitializationUnitId,
    unit: &InitializationUnit,
    failure_roots: &Arena<InitializationFailureRoot>,
    functions: &Arena<Function>,
    seen_failure_roots: &mut [bool],
    seen_functions: &mut HashSet<crate::FunctionId>,
) -> Result<(), HirInitializationUnitIdentityError> {
    let unit_index = raw_index(unit_id);
    let failure_root_index = local_index(unit.failure_root);
    if failure_root_index >= failure_roots.len() {
        return Err(HirInitializationUnitIdentityError::UnknownReference {
            unit: unit_index,
            relation: InitializationRelation::FailureRoot,
            target: raw_index(unit.failure_root),
        });
    }
    if seen_failure_roots[failure_root_index] {
        return Err(HirInitializationUnitIdentityError::DuplicateReference {
            unit: unit_index,
            relation: InitializationRelation::FailureRoot,
            target: raw_index(unit.failure_root),
        });
    }
    seen_failure_roots[failure_root_index] = true;
    if failure_roots[unit.failure_root].unit != unit_id {
        return Err(HirInitializationUnitIdentityError::BackReference {
            unit: unit_index,
            relation: InitializationRelation::FailureRoot,
            target: raw_index(unit.failure_root),
        });
    }

    for (relation, function) in [
        (InitializationRelation::Initializer, unit.initializer),
        (InitializationRelation::Ensure, unit.ensure),
    ] {
        if local_index(function) >= functions.len() {
            return Err(HirInitializationUnitIdentityError::UnknownReference {
                unit: unit_index,
                relation,
                target: raw_index(function),
            });
        }
        if !seen_functions.insert(function) {
            return Err(HirInitializationUnitIdentityError::DuplicateReference {
                unit: unit_index,
                relation,
                target: raw_index(function),
            });
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn eager_property_key(
    unit_id: InitializationUnitId,
    property_id: crate::PropertyId,
    storage_id: crate::GlobalId,
    globals: &Arena<Global>,
    properties: &Arena<Property>,
    delegate_storages: &Arena<DelegateStorage>,
    property_identities: &HirPropertyIdentities,
) -> Result<InitializationUnitKey, HirInitializationUnitIdentityError> {
    let unit = raw_index(unit_id);
    if local_index(property_id) >= properties.len() {
        return Err(HirInitializationUnitIdentityError::UnknownReference {
            unit,
            relation: InitializationRelation::Property,
            target: raw_index(property_id),
        });
    }
    if local_index(storage_id) >= globals.len() {
        return Err(HirInitializationUnitIdentityError::UnknownReference {
            unit,
            relation: InitializationRelation::GlobalStorage,
            target: raw_index(storage_id),
        });
    }
    let property = &properties[property_id];
    let global = &globals[storage_id];
    if global.property != property_id
        || !matches!(
            global.storage,
            GlobalStorage::Managed {
                state: HirStaticInitialState::ZeroedForRuntimeUnit { unit }
            } if unit == unit_id
        )
    {
        return Err(HirInitializationUnitIdentityError::StorageRelation { unit });
    }
    match &property.representation {
        PropertyRepresentation::Stored(stored)
            if matches!(
                stored.backing,
                PropertyBacking::TopLevelGlobal {
                    storage,
                    initialization: TopLevelInitialization::Runtime(initialization),
                } if storage == storage_id && initialization == unit_id
            ) => {}
        PropertyRepresentation::Delegated { storage } => {
            if local_index(*storage) >= delegate_storages.len() {
                return Err(HirInitializationUnitIdentityError::UnknownReference {
                    unit,
                    relation: InitializationRelation::DelegateStorage,
                    target: raw_index(*storage),
                });
            }
            let delegate = &delegate_storages[*storage];
            if delegate.property != property_id
                || delegate.location != DelegateStorageLocation::ManagedGlobal(storage_id)
            {
                return Err(HirInitializationUnitIdentityError::StorageRelation { unit });
            }
        }
        _ => return Err(HirInitializationUnitIdentityError::StorageRelation { unit }),
    }

    match (property.owner, &property_identities[property_id]) {
        (PropertyOwner::TopLevel, HirPropertyIdentity::Ordinary(record)) => {
            Ok(InitializationUnitKey::TopLevelProperty(record.id()))
        }
        (PropertyOwner::Extension(_), HirPropertyIdentity::Extension(record)) => {
            Ok(InitializationUnitKey::ExtensionProperty(record.id()))
        }
        _ => Err(HirInitializationUnitIdentityError::PropertyKind { unit }),
    }
}

#[allow(clippy::too_many_arguments)]
fn singleton_key(
    unit_id: InitializationUnitId,
    value_id: crate::SingletonValueId,
    published_root_id: crate::SingletonPublishedRootId,
    objects: &Arena<ObjectDecl>,
    companions: &Arena<CompanionRelation>,
    singleton_values: &Arena<SingletonValue>,
    published_roots: &Arena<SingletonPublishedRoot>,
    nominal_identities: &HirNominalIdentities,
    seen_values: &mut [bool],
    seen_published_roots: &mut [bool],
) -> Result<InitializationUnitKey, HirInitializationUnitIdentityError> {
    let unit = raw_index(unit_id);
    mark_reference(
        unit,
        InitializationRelation::SingletonValue,
        value_id,
        seen_values,
    )?;
    mark_reference(
        unit,
        InitializationRelation::SingletonPublishedRoot,
        published_root_id,
        seen_published_roots,
    )?;
    let value = &singleton_values[value_id];
    let root = &published_roots[published_root_id];
    if value.initialization != unit_id
        || value.published_root != published_root_id
        || root.value != value_id
    {
        return Err(HirInitializationUnitIdentityError::SingletonRelation { unit });
    }
    if local_index(value.declaration) >= objects.len() {
        return Err(HirInitializationUnitIdentityError::UnknownReference {
            unit,
            relation: InitializationRelation::Object,
            target: raw_index(value.declaration),
        });
    }
    let object = &objects[value.declaration];
    if object.singleton_value != value_id {
        return Err(HirInitializationUnitIdentityError::SingletonRelation { unit });
    }
    let identity = &nominal_identities[value.declaration];
    let persistent_type = identity.concrete_type_id();
    match object.kind {
        ObjectKind::Standalone => persistent_type
            .map(InitializationUnitKey::Object)
            .ok_or(HirInitializationUnitIdentityError::ObjectIdentity { unit }),
        ObjectKind::Companion(companion) => {
            if local_index(companion) >= companions.len() {
                return Err(HirInitializationUnitIdentityError::UnknownReference {
                    unit,
                    relation: InitializationRelation::Companion,
                    target: raw_index(companion),
                });
            }
            if companions[companion].object != value.declaration {
                return Err(HirInitializationUnitIdentityError::CompanionRelation { unit });
            }
            match identity.declaration_id() {
                crate::SourceNominalId::Concrete(id) => Ok(InitializationUnitKey::Companion(id)),
                crate::SourceNominalId::GenericTemplate(id) => {
                    Ok(InitializationUnitKey::GenericCompanionTemplate(id))
                }
            }
        }
    }
}

fn mark_reference<T>(
    unit: u32,
    relation: InitializationRelation,
    id: Idx<T>,
    seen: &mut [bool],
) -> Result<(), HirInitializationUnitIdentityError> {
    let index = local_index(id);
    if index >= seen.len() {
        return Err(HirInitializationUnitIdentityError::UnknownReference {
            unit,
            relation,
            target: raw_index(id),
        });
    }
    if seen[index] {
        return Err(HirInitializationUnitIdentityError::DuplicateReference {
            unit,
            relation,
            target: raw_index(id),
        });
    }
    seen[index] = true;
    Ok(())
}

fn require_coverage(
    relation: InitializationRelation,
    seen: &[bool],
) -> Result<(), HirInitializationUnitIdentityError> {
    if let Some(index) = seen.iter().position(|seen| !seen) {
        Err(HirInitializationUnitIdentityError::UnownedReference {
            relation,
            target: index as u32,
        })
    } else {
        Ok(())
    }
}

fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

fn raw_index<T>(id: Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
