use scoop_identity::{
    CallableBodyKey, ConeIdentity, GeneratedBridgeUnitKey, IdentityLayer, ValidatedIdentityGraph,
};
use scoop_wire::{BudgetMeter, WireEncode, WirePath, encode_canonical_temporary_with_meter};

use super::*;

mod error;
pub use error::{
    BridgeRelationError, LirFoundationOwnershipError, LirFoundationValidationError,
    NativeContractRelationError, SafepointRelationError,
};

mod relations;
use relations::{BridgeTables, validate_bridges, validate_native_contracts, validate_safepoints};

/// LIR foundation reconstructed from validated identities and checked
/// non-identity relations.
#[derive(Debug)]
pub struct ValidatedLirFoundation {
    producer: ConeIdentity,
    canonical: CanonicalLirFoundation,
}

impl ValidatedLirFoundation {
    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn counts(&self) -> LirFoundationCounts {
        self.canonical.counts()
    }

    pub(crate) fn into_canonical(self) -> CanonicalLirFoundation {
        self.canonical
    }

    #[doc(hidden)]
    pub fn native_contracts(&self) -> &[NativeExternalContractRecord] {
        &self.canonical.native_contracts
    }

    #[doc(hidden)]
    pub fn c_abi_signatures(&self) -> &[CanonicalCAbiSignatureFingerprintRecord] {
        &self.canonical.c_abi_signatures
    }

    #[doc(hidden)]
    pub fn c_abi_layouts(&self) -> &[CanonicalCAbiLayoutFingerprintRecord] {
        &self.canonical.c_abi_layouts
    }

    #[doc(hidden)]
    pub fn callback_bridges(&self) -> &[CallbackBridgeRecord] {
        &self.canonical.callback_bridges
    }
}

impl WireEncode for ValidatedLirFoundation {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.canonical.encode(encoder)
    }
}

impl DecodedLirFoundation {
    pub fn validate(
        self,
        producer: ConeIdentity,
        identities: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<ValidatedLirFoundation, LirFoundationValidationError> {
        validate_foundation(self, producer, identities, meter)
    }
}

fn validate_foundation(
    foundation: DecodedLirFoundation,
    producer: ConeIdentity,
    identities: &mut ValidatedIdentityGraph,
    meter: &mut BudgetMeter,
) -> Result<ValidatedLirFoundation, LirFoundationValidationError> {
    let original = encode_canonical_temporary_with_meter(&foundation, meter, &WirePath::root())
        .map_err(LirFoundationValidationError::Resource)?;
    let DecodedLirFoundationWire {
        exact_types: _,
        layouts: _,
        scans: _,
        dispatch_tables: _,
        static_storages: _,
        immortal_objects: _,
        odr_groups: _,
        odr_members: _,
        callable_bodies: _,
        safepoint_sites: _,
        runtime_types,
        safepoints,
        symbol_requests,
        native_contracts,
        c_abi_signatures,
        c_abi_layouts,
        bridge_units: _,
        bridge_atoms: _,
        callback_bridges,
        native_link_requirements: _,
        definition_plans: _,
        definition_atoms: _,
    } = foundation.decoded;

    macro_rules! records {
        ($field:literal, $id:ty, $key:ty) => {
            identities
                .records::<$id, $key>(IdentityLayer::Lir, meter, &WirePath::root().field($field))
                .map_err(LirFoundationValidationError::Identity)?
        };
    }
    let exact_types: Vec<ExactTypeRecord> = records!(1, PersistentExactTypeId, ExactTypeKey);
    let layouts: Vec<LayoutRecord> = records!(2, PersistentLayoutId, LayoutKey);
    let scans: Vec<ScanRecord> = records!(3, PersistentScanId, ScanKey);
    let dispatch_tables: Vec<DispatchTableRecord> =
        records!(4, PersistentDispatchTableId, DispatchTableKey);
    let static_storages: Vec<StaticStorageRecord> =
        records!(5, PersistentStaticStorageId, StaticStorageKey);
    let immortal_objects: Vec<ImmortalObjectRecord> =
        records!(6, PersistentImmortalObjectId, ImmortalObjectKey);
    let odr_groups: Vec<OdrGroupRecord> = records!(7, OdrGroupId, SpecializationKey);
    let odr_members: Vec<OdrMemberRecord> = records!(8, OdrMemberId, OdrMemberKey);
    let callable_bodies: Vec<CallableBodyRecord> = identities
        .runtime_records::<PersistentCallableBodyId, CallableBodyKey>(
            IdentityLayer::Lir,
            meter,
            &WirePath::root().field(9),
        )
        .map_err(LirFoundationValidationError::Identity)?;
    let safepoint_sites: Vec<SafepointSiteRecord> =
        records!(10, PersistentSafepointSiteId, SafepointSiteKey);
    let bridge_units: Vec<BridgeUnitRecord> =
        records!(17, GeneratedBridgeUnitId, GeneratedBridgeUnitKey);
    let bridge_atoms: Vec<BridgeAtomRecord> =
        records!(18, GeneratedBridgeAtomId, GeneratedBridgeAtomKey);
    let native_link_requirements: Vec<NativeLinkRequirementRecord> =
        records!(20, NativeLinkRequirementId, NativeLinkRequirementKey);
    let definition_plans: Vec<DefinitionPlanRecord> =
        records!(21, ObjectDefinitionPlanId, ObjectDefinitionPlanKey);
    let definition_atoms: Vec<DefinitionAtomRecord> =
        records!(22, ObjectDefinitionAtomId, ObjectDefinitionAtomKey);

    let mut resolved_runtime_types = Vec::new();
    meter
        .try_reserve_collection_slots(
            &mut resolved_runtime_types,
            runtime_types.len(),
            &WirePath::root().field(11),
        )
        .map_err(LirFoundationValidationError::Resource)?;
    for (index, record) in runtime_types.into_iter().enumerate() {
        let path = WirePath::root()
            .field(11)
            .key("exact-type", *record.decoded_exact_type().as_array());
        resolved_runtime_types.push(
            record
                .resolve(identities, meter, &path)
                .map_err(|error| LirFoundationValidationError::RuntimeType { index, error })?,
        );
    }

    let mut resolved_safepoints = Vec::new();
    meter
        .try_reserve_collection_slots(
            &mut resolved_safepoints,
            safepoints.len(),
            &WirePath::root().field(12),
        )
        .map_err(LirFoundationValidationError::Resource)?;
    for (index, record) in safepoints.into_iter().enumerate() {
        let path = WirePath::root()
            .field(12)
            .key("safepoint-site", *record.decoded_site().as_array());
        resolved_safepoints.push(
            record
                .resolve(identities, meter, &path)
                .map_err(|error| LirFoundationValidationError::Safepoint { index, error })?,
        );
    }

    let symbol_requests = symbol_requests
        .resolve(identities)
        .map_err(LirFoundationValidationError::SymbolRequests)?;

    let mut resolved_signatures = Vec::new();
    meter
        .try_reserve_collection_slots(
            &mut resolved_signatures,
            c_abi_signatures.len(),
            &WirePath::root().field(15),
        )
        .map_err(LirFoundationValidationError::Resource)?;
    for (index, record) in c_abi_signatures.into_iter().enumerate() {
        resolved_signatures.push(
            record
                .resolve_verified(identities)
                .map_err(|error| LirFoundationValidationError::CAbiSignature { index, error })?,
        );
    }

    let mut resolved_layouts = Vec::new();
    meter
        .try_reserve_collection_slots(
            &mut resolved_layouts,
            c_abi_layouts.len(),
            &WirePath::root().field(16),
        )
        .map_err(LirFoundationValidationError::Resource)?;
    for (index, record) in c_abi_layouts.into_iter().enumerate() {
        resolved_layouts.push(
            record
                .resolve_verified(identities)
                .map_err(|error| LirFoundationValidationError::CAbiLayout { index, error })?,
        );
    }

    let mut resolved_contracts = Vec::new();
    meter
        .try_reserve_collection_slots(
            &mut resolved_contracts,
            native_contracts.len(),
            &WirePath::root().field(14),
        )
        .map_err(LirFoundationValidationError::Resource)?;
    for (index, record) in native_contracts.into_iter().enumerate() {
        resolved_contracts.push(
            record
                .resolve_verified(identities)
                .map_err(|error| LirFoundationValidationError::NativeContract { index, error })?,
        );
    }

    let mut resolved_callbacks = Vec::new();
    meter
        .try_reserve_collection_slots(
            &mut resolved_callbacks,
            callback_bridges.len(),
            &WirePath::root().field(19),
        )
        .map_err(LirFoundationValidationError::Resource)?;
    for (index, record) in callback_bridges.into_iter().enumerate() {
        resolved_callbacks.push(
            record
                .resolve(identities)
                .map_err(|error| LirFoundationValidationError::CallbackBridge { index, error })?,
        );
    }

    validate_safepoints(
        &callable_bodies,
        &safepoint_sites,
        &resolved_safepoints,
        meter,
    )?;
    validate_native_contracts(identities, &resolved_contracts, meter)?;
    validate_bridges(
        identities,
        producer,
        BridgeTables {
            contracts: &resolved_contracts,
            signatures: &resolved_signatures,
            layouts: &resolved_layouts,
            units: &bridge_units,
            atoms: &bridge_atoms,
            callbacks: &resolved_callbacks,
            plans: &definition_plans,
        },
        meter,
    )?;

    let mut canonical = CanonicalLirFoundation::empty();
    macro_rules! set {
        ($method:ident, $records:expr) => {
            canonical
                .$method($records)
                .map_err(LirFoundationValidationError::Build)?
        };
    }
    set!(set_exact_types, exact_types);
    set!(set_layouts, layouts);
    set!(set_scans, scans);
    set!(set_dispatch_tables, dispatch_tables);
    set!(set_static_storages, static_storages);
    set!(set_immortal_objects, immortal_objects);
    set!(set_odr_groups, odr_groups);
    set!(set_odr_members, odr_members);
    set!(set_callable_bodies, callable_bodies);
    set!(set_safepoint_sites, safepoint_sites);
    set!(set_runtime_types, resolved_runtime_types);
    set!(set_safepoints, resolved_safepoints);
    canonical.set_symbol_requests(symbol_requests);
    set!(set_native_contracts, resolved_contracts);
    set!(set_c_abi_signatures, resolved_signatures);
    set!(set_c_abi_layouts, resolved_layouts);
    set!(set_bridge_units, bridge_units);
    set!(set_bridge_atoms, bridge_atoms);
    set!(set_callback_bridges, resolved_callbacks);
    set!(set_native_link_requirements, native_link_requirements);
    set!(set_definition_plans, definition_plans);
    set!(set_definition_atoms, definition_atoms);

    let rebuilt = encode_canonical_temporary_with_meter(&canonical, meter, &WirePath::root())
        .map_err(LirFoundationValidationError::Resource)?;
    if rebuilt != original {
        return Err(LirFoundationValidationError::NonCanonicalFoundation);
    }
    Ok(ValidatedLirFoundation {
        producer,
        canonical,
    })
}

#[cfg(test)]
mod tests;
