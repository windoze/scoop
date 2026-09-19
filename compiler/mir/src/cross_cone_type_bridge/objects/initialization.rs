use super::*;
use scoop_identity::{DefinitionOwnerAtom, PropertyAccessorKey, PropertyOwner};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MirExternalInitializationCauseV1 {
    ObjectValue(PersistentObjectValueId),
    PropertyAccessor(PersistentPropertyAccessorId),
    InitializationSupport(PersistentInitializationUnitId),
}

/// A checked identity relation. Complete sections additionally prove that an
/// actual committed ensure use produced this edge.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SelectedExternalInitializationUseV1 {
    pub(super) local_unit: PersistentInitializationUnitId,
    pub(super) provider: ConeIdentity,
    pub(super) dependency_unit: PersistentInitializationUnitId,
    pub(super) cause: MirExternalInitializationCauseV1,
}
impl SelectedExternalInitializationUseV1 {
    pub fn try_new(
        consumer: ConeIdentity,
        identities: &ValidatedIdentityGraph,
        local_unit: PersistentInitializationUnitId,
        provider: ConeIdentity,
        dependency_unit: PersistentInitializationUnitId,
        cause: MirExternalInitializationCauseV1,
        meter: &mut BudgetMeter,
    ) -> Result<Self, MirObjectBridgeError> {
        meter.charge_nodes(1, &WirePath::root())?;
        meter.charge_work(12, &WirePath::root())?;
        if unit_provider(identities, local_unit)? != consumer {
            return Err(MirObjectBridgeError::LocalUnitOwner { unit: local_unit });
        }
        if provider == consumer || unit_provider(identities, dependency_unit)? != provider {
            return Err(MirObjectBridgeError::DependencyProvider {
                unit: dependency_unit,
            });
        }
        let key = identities.canonical_key::<_, InitializationUnitKey>(dependency_unit)?;
        let valid = match cause {
            MirExternalInitializationCauseV1::InitializationSupport(unit) => {
                unit == dependency_unit
            }
            MirExternalInitializationCauseV1::ObjectValue(value) => {
                let object = identities.canonical_key::<_, SourceDeclarationKey>(value)?;
                match key.as_ref() {
                    InitializationUnitKey::Object(nominal)
                    | InitializationUnitKey::Companion(nominal) => {
                        super::validation::source_keys_equal(
                            identities
                                .canonical_key::<_, SourceDeclarationKey>(*nominal)?
                                .as_ref(),
                            &object,
                            meter,
                        )?
                    }
                    _ => false,
                }
            }
            MirExternalInitializationCauseV1::PropertyAccessor(accessor) => {
                let accessor = identities.canonical_key::<_, PropertyAccessorKey>(accessor)?;
                property_unit(identities, accessor.owner(), key.as_ref())?
            }
        };
        if !valid {
            return Err(MirObjectBridgeError::CauseUnitMismatch);
        }
        Ok(Self {
            local_unit,
            provider,
            dependency_unit,
            cause,
        })
    }
    pub const fn local_unit(self) -> PersistentInitializationUnitId {
        self.local_unit
    }
    pub const fn provider(self) -> ConeIdentity {
        self.provider
    }
    pub const fn dependency_unit(self) -> PersistentInitializationUnitId {
        self.dependency_unit
    }
    pub const fn cause(self) -> MirExternalInitializationCauseV1 {
        self.cause
    }
}

fn unit_provider(
    identities: &ValidatedIdentityGraph,
    unit: PersistentInitializationUnitId,
) -> Result<ConeIdentity, MirObjectBridgeError> {
    let key = identities.canonical_key::<_, InitializationUnitKey>(unit)?;
    let source = match key.as_ref() {
        InitializationUnitKey::TopLevelProperty(id) => {
            let source = identities.canonical_key::<_, SourceDeclarationKey>(*id)?;
            if !source.owners().owners().is_empty() {
                return Err(MirObjectBridgeError::UnitIdentity);
            }
            source
        }
        InitializationUnitKey::ExtensionProperty(id) => {
            identities.canonical_key::<_, SourceDeclarationKey>(*id)?
        }
        InitializationUnitKey::Object(id) | InitializationUnitKey::Companion(id) => {
            let source = identities.canonical_key::<_, SourceDeclarationKey>(*id)?;
            if source.declaration_kind() != scoop_identity::SourceDeclarationKind::Object {
                return Err(MirObjectBridgeError::UnitIdentity);
            }
            source
        }
        InitializationUnitKey::GenericDelegatedExtensionApplication { .. } => {
            return Err(MirObjectBridgeError::GenericUnitGate { unit });
        }
    };
    Ok(source.origin())
}

fn property_unit(
    identities: &ValidatedIdentityGraph,
    property: PropertyOwner,
    unit: &InitializationUnitKey,
) -> Result<bool, MirObjectBridgeError> {
    if matches!((property, unit), (PropertyOwner::Property(id), InitializationUnitKey::TopLevelProperty(other)) if id == *other)
        || matches!((property, unit), (PropertyOwner::ExtensionProperty(id), InitializationUnitKey::ExtensionProperty(other)) if id == *other)
    {
        return Ok(true);
    }
    let (InitializationUnitKey::Object(object) | InitializationUnitKey::Companion(object)) = unit
    else {
        return Ok(false);
    };
    let source = match property {
        PropertyOwner::Property(id) => identities.canonical_key::<_, SourceDeclarationKey>(id)?,
        PropertyOwner::ExtensionProperty(id) => {
            identities.canonical_key::<_, SourceDeclarationKey>(id)?
        }
    };
    Ok(
        matches!(source.owners().owners().last(), Some(DefinitionOwnerAtom::Type(owner)) if owner == object),
    )
}
