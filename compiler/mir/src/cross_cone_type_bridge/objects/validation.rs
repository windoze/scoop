use super::*;

#[derive(Debug)]
pub enum MirObjectBridgeError {
    Identity(IdentityReferenceError),
    Resource(WireError),
    SourceEncoding(scoop_wire::cbor::EncodeError),
    MissingType {
        exact: PersistentExactTypeId,
    },
    MissingEnsure {
        target: StrongCallableDefinitionOwner,
    },
    ObjectIdentity,
    BackingIdentity,
    UnitIdentity,
    EnsureIdentity,
    GenericUnitGate {
        unit: PersistentInitializationUnitId,
    },
    LocalUnitOwner {
        unit: PersistentInitializationUnitId,
    },
    DependencyProvider {
        unit: PersistentInitializationUnitId,
    },
    CauseUnitMismatch,
    DuplicateObject {
        value: PersistentObjectValueId,
    },
    NonCanonicalObjectOrder {
        index: usize,
    },
    DuplicateInitializationUse,
    DuplicateInitializationDependency,
    InitializationDependencyInventory,
    NonCanonicalInitializationUseOrder {
        index: usize,
    },
}
impl From<IdentityReferenceError> for MirObjectBridgeError {
    fn from(value: IdentityReferenceError) -> Self {
        Self::Identity(value)
    }
}
impl From<WireError> for MirObjectBridgeError {
    fn from(value: WireError) -> Self {
        Self::Resource(value)
    }
}
impl std::fmt::Display for MirObjectBridgeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "MIR object bridge: {self:?}")
    }
}
impl std::error::Error for MirObjectBridgeError {}

impl MirObjectBridgeAuthority<'_> {
    pub(in crate::cross_cone_type_bridge) fn validate_object(
        &self,
        value: PersistentObjectValueId,
        backing: PersistentExactTypeId,
        unit: PersistentInitializationUnitId,
        ensure: StrongCallableDefinitionOwner,
        read: MirObjectValueReadPlanV1,
    ) -> Result<ConeIdentity, MirObjectBridgeError> {
        let source = self
            .identities
            .canonical_key::<_, SourceDeclarationKey>(value)?;
        let object = self
            .types
            .get(read.object())
            .ok_or(MirObjectBridgeError::MissingType {
                exact: read.object(),
            })?;
        let MirTypeOriginV1::SourceNominal(nominal) = object.origin() else {
            return Err(MirObjectBridgeError::ObjectIdentity);
        };
        if !source_keys_equal(
            self.identities
                .canonical_key::<_, SourceDeclarationKey>(*nominal)?
                .as_ref(),
            &source,
        ) || object.representation() != &(MirTypeRepresentationV1::Object { backing })
        {
            return Err(MirObjectBridgeError::ObjectIdentity);
        }
        let backing_type = self
            .types
            .get(backing)
            .ok_or(MirObjectBridgeError::MissingType { exact: backing })?;
        if !matches!(backing_type.origin(), MirTypeOriginV1::GeneratedNominal { role: GeneratedNominalKey::ObjectBackingClass { object }, .. } if object == nominal)
            || !matches!(
                backing_type.representation(),
                MirTypeRepresentationV1::ObjectBacking { .. }
            )
        {
            return Err(MirObjectBridgeError::BackingIdentity);
        }
        let unit_key = self
            .identities
            .canonical_key::<_, InitializationUnitKey>(unit)?;
        if !matches!(unit_key.as_ref(), InitializationUnitKey::Object(owner) | InitializationUnitKey::Companion(owner) if owner == nominal)
        {
            return Err(MirObjectBridgeError::UnitIdentity);
        }
        let binding = self
            .callables
            .get(ensure.into())
            .ok_or(MirObjectBridgeError::MissingEnsure { target: ensure })?;
        if !matches!(binding.origin().as_ref(), MirCallableOriginV1::Generated { role: GeneratedCallableKey::Initialization { unit: owner, role: InitializationCallableRole::Ensure }, .. } if *owner == unit)
            || binding.lowering_role() != &(MirCallableLoweringRoleV1::ObjectEnsure { unit })
        {
            return Err(MirObjectBridgeError::EnsureIdentity);
        }
        Ok(source.origin())
    }
}

pub(super) fn source_keys_equal(left: &SourceDeclarationKey, right: &SourceDeclarationKey) -> bool {
    left == right
}
