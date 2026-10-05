use super::*;

/// An export whose exact origin and ordered representation have been checked
/// against the provider's identity graph and MIR generated identity foundation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamFreeMirTypeExportV1 {
    exact: PersistentExactTypeId,
    origin: MirTypeOriginV1,
    odr: bool,
    facts: MirTypeFactsV1,
    representation: MirTypeRepresentationV1,
    base_and_interfaces: MirBaseAndInterfacesV1,
}
impl ParamFreeMirTypeExportV1 {
    pub fn try_new(
        authority: MirTypeBridgeAuthority<'_>,
        exact: PersistentExactTypeId,
        origin: MirTypeOriginV1,
        facts: MirTypeFactsV1,
        representation: MirTypeRepresentationV1,
        base_and_interfaces: MirBaseAndInterfacesV1,
    ) -> Result<Self, MirTypeBridgeError> {
        let record = Self {
            exact,
            odr: is_odr_origin(&origin, authority)?,
            origin,
            facts,
            representation,
            base_and_interfaces,
        };
        authority.validate(&record)?;
        Ok(record)
    }
    pub const fn exact(&self) -> PersistentExactTypeId {
        self.exact
    }
    pub const fn origin(&self) -> &MirTypeOriginV1 {
        &self.origin
    }
    pub(in crate::cross_cone_type_bridge) const fn is_odr(&self) -> bool {
        self.odr
    }
    pub const fn facts(&self) -> MirTypeFactsV1 {
        self.facts
    }
    pub const fn representation(&self) -> &MirTypeRepresentationV1 {
        &self.representation
    }
    pub const fn base_and_interfaces(&self) -> &MirBaseAndInterfacesV1 {
        &self.base_and_interfaces
    }
}

fn is_odr_origin(
    origin: &MirTypeOriginV1,
    authority: MirTypeBridgeAuthority<'_>,
) -> Result<bool, MirTypeBridgeError> {
    match origin {
        MirTypeOriginV1::SourceNominal(_) => Ok(false),
        MirTypeOriginV1::NominalApplication(_) => Ok(true),
        MirTypeOriginV1::GeneratedNominal { role, .. } => {
            let payload = match role {
                GeneratedNominalKey::BoxedValue { payload } => *payload,
                GeneratedNominalKey::CoroutineStep { result } => *result,
                GeneratedNominalKey::CoroutineSlot { value } => *value,
                GeneratedNominalKey::TaskContext(_)
                | GeneratedNominalKey::ObjectBackingClass { .. }
                | GeneratedNominalKey::GenericObjectBackingClass { .. }
                | GeneratedNominalKey::ClosureEnvironment { .. }
                | GeneratedNominalKey::CallableAdapterEnvironment { .. }
                | GeneratedNominalKey::CoroutineFrame { .. }
                | GeneratedNominalKey::ContinuationAdapterEnvironment { .. } => return Ok(false),
            };
            let key = authority
                .identities
                .canonical_key::<_, ExactTypeKey>(payload)?;
            if let ExactTypeKey::Nominal(nominal) = key.as_ref() {
                return Ok(matches!(role, GeneratedNominalKey::CoroutineSlot { .. })
                    && matches!(authority.foundation.generated_type_key(*nominal),
                        Some(GeneratedNominalKey::TaskContext(storage))
                            if storage.role == crate::ContextStorageRole::Mark));
            }
            Ok(true)
        }
    }
}

/// Explicit, read-only semantic authority; callers cannot replace absent
/// generated keys with coincidentally equal storage shapes.
#[derive(Clone, Copy)]
pub struct MirTypeBridgeAuthority<'a> {
    pub identities: &'a ValidatedIdentityGraph,
    pub foundation: &'a crate::CanonicalMirFoundation,
}
