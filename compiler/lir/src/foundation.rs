//! Arena-independent canonical projection of the LIR identity foundation.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    CanonicalCAbiLayoutFingerprintRecord, CanonicalCAbiSignatureFingerprintRecord,
    CborIdentityRecord, ConeImageSupportRole, DecodedCallableBodyKey, DecodedCallableBodyKeyKind,
    DefinitionAtomRole, DefinitionAtomSubkey, DispatchTableKey, GeneratedBridgeAtomId,
    GeneratedBridgeAtomKey, GeneratedBridgeAtomRoleKey, GeneratedBridgeUnitId,
    GeneratedBridgeUnitKey, ImmortalObjectKey, LayoutKey, LinkageClass,
    NativeExternalContractRecord, NativeLinkRequirementId, NativeLinkRequirementKey,
    ObjectDefinitionAtomId, ObjectDefinitionAtomKey, ObjectDefinitionIdentityError,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, OdrGroupId, OdrMemberId, OdrMemberKey,
    PersistentCallableBodyId, PersistentDispatchTableId, PersistentExactTypeId, PersistentId,
    PersistentImmortalObjectId, PersistentLayoutId, PersistentSafepointSiteId, PersistentScanId,
    PersistentStaticStorageId, PersistentSymbolRequest, RuntimeIdentityRecord, SafepointSiteKey,
    ScanKey, SpecializationKey, StaticStorageKey, StrongDefinitionEntity, StrongDefinitionRole,
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_wire::{Encoder, HashError, RuntimeDecodeError, WireEncode, decode_runtime};

use crate::{CallbackBridgeRecord, RuntimeTypeMappingRecord, SafepointMappingRecord};
use scoop_identity::PersistentSymbolRequestTable;

mod wire;
pub use wire::{
    BridgeRelationError, DecodedLirFoundation, LirFoundationOwnershipError,
    LirFoundationValidationError, NativeContractRelationError, SafepointRelationError,
    ValidatedLirFoundation,
};

mod bridge_layouts;
mod imported;
mod projection;
mod strong_profile;
pub use bridge_layouts::GeneratedBridgeLayoutClosureError;
pub(crate) use bridge_layouts::{bridge_unit_keys, required_generated_bridge_layouts};
pub use imported::{
    ImportedCoreLirCallableId, ImportedLirCallableProjectionError, ImportedLirFoundation,
    ImportedLirId, ImportedLirSelectionError, SelectedImportedLirCallable, SelectedImportedLirSet,
};
pub use strong_profile::{
    DefinitionAtomResolutionError, OdrFreeLirFoundation, OdrFreeLirFoundationError,
    OdrFreeLirFoundationProjectionError,
};

type LayoutRecord = CborIdentityRecord<PersistentLayoutId, LayoutKey>;
type ScanRecord = CborIdentityRecord<PersistentScanId, ScanKey>;
type DispatchTableRecord = CborIdentityRecord<PersistentDispatchTableId, DispatchTableKey>;
type StaticStorageRecord = CborIdentityRecord<PersistentStaticStorageId, StaticStorageKey>;
type ImmortalObjectRecord = CborIdentityRecord<PersistentImmortalObjectId, ImmortalObjectKey>;
type OdrGroupRecord = CborIdentityRecord<OdrGroupId, SpecializationKey>;
type OdrMemberRecord = CborIdentityRecord<OdrMemberId, OdrMemberKey>;
type CallableBodyRecord = RuntimeIdentityRecord<PersistentCallableBodyId>;
type SafepointSiteRecord = CborIdentityRecord<PersistentSafepointSiteId, SafepointSiteKey>;
pub(crate) type BridgeUnitRecord =
    CborIdentityRecord<GeneratedBridgeUnitId, GeneratedBridgeUnitKey>;
pub(crate) type BridgeAtomRecord =
    CborIdentityRecord<GeneratedBridgeAtomId, GeneratedBridgeAtomKey>;
pub(crate) type NativeLinkRequirementRecord =
    CborIdentityRecord<NativeLinkRequirementId, NativeLinkRequirementKey>;
pub(crate) type DefinitionPlanRecord =
    CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>;
pub(crate) type DefinitionAtomRecord =
    CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>;

/// Canonical, arena-independent LIR identity tables produced by lowering.
///
/// Every table is complete even when empty. Setters canonicalize trusted
/// producer input before replacing a table, so every observable value can be
/// encoded directly without a second sorting pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalLirFoundation {
    materialized_exact_types: Vec<PersistentExactTypeId>,
    layouts: Vec<LayoutRecord>,
    scans: Vec<ScanRecord>,
    dispatch_tables: Vec<DispatchTableRecord>,
    static_storages: Vec<StaticStorageRecord>,
    immortal_objects: Vec<ImmortalObjectRecord>,
    odr_groups: Vec<OdrGroupRecord>,
    odr_members: Vec<OdrMemberRecord>,
    callable_bodies: Vec<CallableBodyRecord>,
    safepoint_sites: Vec<SafepointSiteRecord>,
    runtime_types: Vec<RuntimeTypeMappingRecord>,
    safepoints: Vec<SafepointMappingRecord>,
    symbol_requests: PersistentSymbolRequestTable,
    native_contracts: Vec<NativeExternalContractRecord>,
    c_abi_signatures: Vec<CanonicalCAbiSignatureFingerprintRecord>,
    c_abi_layouts: Vec<CanonicalCAbiLayoutFingerprintRecord>,
    bridge_units: Vec<BridgeUnitRecord>,
    bridge_atoms: Vec<BridgeAtomRecord>,
    callback_bridges: Vec<CallbackBridgeRecord>,
    native_link_requirements: Vec<NativeLinkRequirementRecord>,
    definition_plans: Vec<DefinitionPlanRecord>,
    definition_atoms: Vec<DefinitionAtomRecord>,
}

impl CanonicalLirFoundation {
    pub fn empty() -> Self {
        Self {
            materialized_exact_types: Vec::new(),
            layouts: Vec::new(),
            scans: Vec::new(),
            dispatch_tables: Vec::new(),
            static_storages: Vec::new(),
            immortal_objects: Vec::new(),
            odr_groups: Vec::new(),
            odr_members: Vec::new(),
            callable_bodies: Vec::new(),
            safepoint_sites: Vec::new(),
            runtime_types: Vec::new(),
            safepoints: Vec::new(),
            symbol_requests: PersistentSymbolRequestTable::empty(),
            native_contracts: Vec::new(),
            c_abi_signatures: Vec::new(),
            c_abi_layouts: Vec::new(),
            bridge_units: Vec::new(),
            bridge_atoms: Vec::new(),
            callback_bridges: Vec::new(),
            native_link_requirements: Vec::new(),
            definition_plans: Vec::new(),
            definition_atoms: Vec::new(),
        }
    }

    pub fn counts(&self) -> LirFoundationCounts {
        LirFoundationCounts {
            materialized_exact_types: self.materialized_exact_types.len(),
            layouts: self.layouts.len(),
            scans: self.scans.len(),
            dispatch_tables: self.dispatch_tables.len(),
            static_storages: self.static_storages.len(),
            immortal_objects: self.immortal_objects.len(),
            odr_groups: self.odr_groups.len(),
            odr_members: self.odr_members.len(),
            callable_bodies: self.callable_bodies.len(),
            safepoint_sites: self.safepoint_sites.len(),
            runtime_types: self.runtime_types.len(),
            safepoints: self.safepoints.len(),
            symbol_requests: self.symbol_requests.requests().len(),
            native_contracts: self.native_contracts.len(),
            c_abi_signatures: self.c_abi_signatures.len(),
            c_abi_layouts: self.c_abi_layouts.len(),
            bridge_units: self.bridge_units.len(),
            bridge_atoms: self.bridge_atoms.len(),
            callback_bridges: self.callback_bridges.len(),
            native_link_requirements: self.native_link_requirements.len(),
            definition_plans: self.definition_plans.len(),
            definition_atoms: self.definition_atoms.len(),
        }
    }

    pub fn set_materialized_exact_types(
        &mut self,
        exact_types: Vec<PersistentExactTypeId>,
    ) -> Result<(), LirFoundationBuildError> {
        self.materialized_exact_types = sort_unique(
            exact_types,
            LirFoundationTable::MaterializedExactType,
            |exact| *exact,
        )?;
        Ok(())
    }

    pub fn set_layouts(
        &mut self,
        records: Vec<LayoutRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.layouts = sort_unique(records, LirFoundationTable::Layout, CborIdentityRecord::id)?;
        Ok(())
    }

    pub fn set_scans(&mut self, records: Vec<ScanRecord>) -> Result<(), LirFoundationBuildError> {
        self.scans = sort_unique(records, LirFoundationTable::Scan, CborIdentityRecord::id)?;
        Ok(())
    }

    pub fn set_dispatch_tables(
        &mut self,
        records: Vec<DispatchTableRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.dispatch_tables = sort_unique(
            records,
            LirFoundationTable::DispatchTable,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }

    pub fn set_static_storages(
        &mut self,
        records: Vec<StaticStorageRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.static_storages = sort_unique(
            records,
            LirFoundationTable::StaticStorage,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }

    pub fn set_immortal_objects(
        &mut self,
        records: Vec<ImmortalObjectRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.immortal_objects = sort_unique(
            records,
            LirFoundationTable::ImmortalObject,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }

    pub fn set_odr_groups(
        &mut self,
        records: Vec<OdrGroupRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.odr_groups = sort_unique(
            records,
            LirFoundationTable::OdrGroup,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }

    pub fn set_odr_members(
        &mut self,
        records: Vec<OdrMemberRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.odr_members = sort_unique(
            records,
            LirFoundationTable::OdrMember,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }

    pub fn set_callable_bodies(
        &mut self,
        records: Vec<CallableBodyRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.callable_bodies = stable_callable_body_order(records)?;
        Ok(())
    }

    pub fn set_safepoint_sites(
        &mut self,
        records: Vec<SafepointSiteRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.safepoint_sites = sort_unique(
            records,
            LirFoundationTable::SafepointSite,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }

    pub fn set_runtime_types(
        &mut self,
        records: Vec<RuntimeTypeMappingRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.runtime_types = sort_mapping(
            records,
            LirFoundationTable::RuntimeTypeMapping,
            |record| record.exact_type(),
            |record| record.runtime_type().get(),
        )?;
        Ok(())
    }

    pub fn set_safepoints(
        &mut self,
        records: Vec<SafepointMappingRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.safepoints = sort_mapping(
            records,
            LirFoundationTable::SafepointMapping,
            |record| record.site(),
            |record| record.safepoint().get(),
        )?;
        Ok(())
    }

    pub fn set_symbol_requests(&mut self, requests: PersistentSymbolRequestTable) {
        self.symbol_requests = requests;
    }

    pub fn set_native_contracts(
        &mut self,
        records: Vec<NativeExternalContractRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.native_contracts = sort_unique(
            records,
            LirFoundationTable::NativeContract,
            NativeExternalContractRecord::source,
        )?;
        Ok(())
    }

    pub fn set_c_abi_signatures(
        &mut self,
        records: Vec<CanonicalCAbiSignatureFingerprintRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.c_abi_signatures = sort_unique(
            records,
            LirFoundationTable::CAbiSignature,
            CanonicalCAbiSignatureFingerprintRecord::fingerprint,
        )?;
        Ok(())
    }

    pub fn set_c_abi_layouts(
        &mut self,
        records: Vec<CanonicalCAbiLayoutFingerprintRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.c_abi_layouts = sort_unique(
            records,
            LirFoundationTable::CAbiLayout,
            CanonicalCAbiLayoutFingerprintRecord::fingerprint,
        )?;
        Ok(())
    }

    pub fn set_bridge_units(
        &mut self,
        records: Vec<BridgeUnitRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.bridge_units = sort_unique(
            records,
            LirFoundationTable::BridgeUnit,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }

    pub fn set_bridge_atoms(
        &mut self,
        records: Vec<BridgeAtomRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.bridge_atoms = sort_unique(
            records,
            LirFoundationTable::BridgeAtom,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }

    pub fn set_callback_bridges(
        &mut self,
        records: Vec<CallbackBridgeRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.callback_bridges =
            sort_unique(records, LirFoundationTable::CallbackBridge, |record| {
                record.application()
            })?;
        Ok(())
    }

    pub fn set_native_link_requirements(
        &mut self,
        records: Vec<NativeLinkRequirementRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.native_link_requirements = sort_unique(
            records,
            LirFoundationTable::NativeLinkRequirement,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }

    pub fn set_definition_plans(
        &mut self,
        records: Vec<DefinitionPlanRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.definition_plans = sort_unique(
            records,
            LirFoundationTable::DefinitionPlan,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }

    pub fn set_definition_atoms(
        &mut self,
        records: Vec<DefinitionAtomRecord>,
    ) -> Result<(), LirFoundationBuildError> {
        self.definition_atoms = sort_unique(
            records,
            LirFoundationTable::DefinitionAtom,
            CborIdentityRecord::id,
        )?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LirFoundationCounts {
    pub materialized_exact_types: usize,
    pub layouts: usize,
    pub scans: usize,
    pub dispatch_tables: usize,
    pub static_storages: usize,
    pub immortal_objects: usize,
    pub odr_groups: usize,
    pub odr_members: usize,
    pub callable_bodies: usize,
    pub safepoint_sites: usize,
    pub runtime_types: usize,
    pub safepoints: usize,
    pub symbol_requests: usize,
    pub native_contracts: usize,
    pub c_abi_signatures: usize,
    pub c_abi_layouts: usize,
    pub bridge_units: usize,
    pub bridge_atoms: usize,
    pub callback_bridges: usize,
    pub native_link_requirements: usize,
    pub definition_plans: usize,
    pub definition_atoms: usize,
}

impl WireEncode for CanonicalLirFoundation {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(22)?;
        encode_table_field(encoder, 1, &self.materialized_exact_types)?;
        encode_table_field(encoder, 2, &self.layouts)?;
        encode_table_field(encoder, 3, &self.scans)?;
        encode_table_field(encoder, 4, &self.dispatch_tables)?;
        encode_table_field(encoder, 5, &self.static_storages)?;
        encode_table_field(encoder, 6, &self.immortal_objects)?;
        encode_table_field(encoder, 7, &self.odr_groups)?;
        encode_table_field(encoder, 8, &self.odr_members)?;
        encode_table_field(encoder, 9, &self.callable_bodies)?;
        encode_table_field(encoder, 10, &self.safepoint_sites)?;
        encode_table_field(encoder, 11, &self.runtime_types)?;
        encode_table_field(encoder, 12, &self.safepoints)?;
        encoder.field(13)?;
        self.symbol_requests.encode(encoder)?;
        encode_table_field(encoder, 14, &self.native_contracts)?;
        encode_table_field(encoder, 15, &self.c_abi_signatures)?;
        encode_table_field(encoder, 16, &self.c_abi_layouts)?;
        encode_table_field(encoder, 17, &self.bridge_units)?;
        encode_table_field(encoder, 18, &self.bridge_atoms)?;
        encode_table_field(encoder, 19, &self.callback_bridges)?;
        encode_table_field(encoder, 20, &self.native_link_requirements)?;
        encode_table_field(encoder, 21, &self.definition_plans)?;
        encode_table_field(encoder, 22, &self.definition_atoms)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LirFoundationTable {
    MaterializedExactType,
    Layout,
    Scan,
    DispatchTable,
    StaticStorage,
    ImmortalObject,
    OdrGroup,
    OdrMember,
    CallableBody,
    SafepointSite,
    RuntimeTypeMapping,
    SafepointMapping,
    NativeContract,
    CAbiSignature,
    CAbiLayout,
    BridgeUnit,
    BridgeAtom,
    CallbackBridge,
    NativeLinkRequirement,
    DefinitionPlan,
    DefinitionAtom,
}

impl LirFoundationTable {
    const fn name(self) -> &'static str {
        match self {
            Self::MaterializedExactType => "materialized exact type reference",
            Self::Layout => "layout",
            Self::Scan => "scan",
            Self::DispatchTable => "dispatch table",
            Self::StaticStorage => "static storage",
            Self::ImmortalObject => "immortal object",
            Self::OdrGroup => "ODR group",
            Self::OdrMember => "ODR member",
            Self::CallableBody => "callable body",
            Self::SafepointSite => "safepoint site",
            Self::RuntimeTypeMapping => "runtime type mapping",
            Self::SafepointMapping => "safepoint mapping",
            Self::NativeContract => "native contract",
            Self::CAbiSignature => "canonical C ABI signature",
            Self::CAbiLayout => "canonical C ABI layout",
            Self::BridgeUnit => "generated bridge unit",
            Self::BridgeAtom => "generated bridge atom",
            Self::CallbackBridge => "callback bridge",
            Self::NativeLinkRequirement => "native link requirement",
            Self::DefinitionPlan => "object definition plan",
            Self::DefinitionAtom => "object definition atom",
        }
    }
}

#[derive(Debug)]
pub enum LirFoundationBuildError {
    CallableBodyKey(RuntimeDecodeError),
    DuplicateIdentity {
        table: LirFoundationTable,
        identity: [u8; 32],
    },
    IdentityCollision {
        table: LirFoundationTable,
        identity: [u8; 32],
    },
    MissingCallableBodyDependency {
        identity: [u8; 32],
        dependency: [u8; 32],
    },
    CallableBodyCycle {
        first: [u8; 32],
    },
    InconsistentCallableBodyGraph {
        identity: [u8; 32],
    },
    DuplicateDerivedId {
        table: LirFoundationTable,
        derived: u64,
    },
    SafepointOwnerMismatch {
        function: usize,
        site: [u8; 32],
        expected: [u8; 32],
        actual: [u8; 32],
    },
    CallableCStringOwnerMissing {
        owner: [u8; 32],
    },
    CallableCStringAtomMismatch {
        owner: [u8; 32],
        atom: [u8; 32],
    },
    CallableRuntimeScans(crate::StrongCallableRuntimeScanPlanError),
    GeneratedBridgeLayouts(GeneratedBridgeLayoutClosureError),
    GeneratedBridgeAtomHash(HashError),
    DefinitionIdentity(ObjectDefinitionIdentityError),
    DefinitionHash(HashError),
    StaticStorageSemantics(crate::StrongStaticStorageSemanticPlanBuildError),
    SymbolRequest(scoop_identity::PersistentSymbolError),
}

impl fmt::Display for LirFoundationBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CallableBodyKey(error) => error.fmt(formatter),
            Self::DuplicateIdentity { table, identity } => write!(
                formatter,
                "duplicate {} identity {}",
                table.name(),
                HexIdentity(identity)
            ),
            Self::IdentityCollision { table, identity } => write!(
                formatter,
                "conflicting {} records share identity {}",
                table.name(),
                HexIdentity(identity)
            ),
            Self::MissingCallableBodyDependency {
                identity,
                dependency,
            } => write!(
                formatter,
                "callable body {} depends on missing callable body {}",
                HexIdentity(identity),
                HexIdentity(dependency)
            ),
            Self::CallableBodyCycle { first } => write!(
                formatter,
                "callable body dependency cycle contains {}",
                HexIdentity(first)
            ),
            Self::InconsistentCallableBodyGraph { identity } => write!(
                formatter,
                "inconsistent callable body dependency graph at {}",
                HexIdentity(identity)
            ),
            Self::DuplicateDerivedId { table, derived } => {
                write!(
                    formatter,
                    "duplicate derived id {derived} in {} table",
                    table.name()
                )
            }
            Self::SafepointOwnerMismatch {
                function,
                site,
                expected,
                actual,
            } => write!(
                formatter,
                "function {function} safepoint site {} belongs to another callable body (actual {}, expected {})",
                HexIdentity(site),
                HexIdentity(actual),
                HexIdentity(expected)
            ),
            Self::CallableCStringOwnerMissing { owner } => write!(
                formatter,
                "callable C string belongs to missing callable body {}",
                HexIdentity(owner)
            ),
            Self::CallableCStringAtomMismatch { owner, atom } => write!(
                formatter,
                "callable C string atom {} does not match callable body {}",
                HexIdentity(atom),
                HexIdentity(owner)
            ),
            Self::CallableRuntimeScans(error) => error.fmt(formatter),
            Self::GeneratedBridgeLayouts(error) => error.fmt(formatter),
            Self::GeneratedBridgeAtomHash(error) => error.fmt(formatter),
            Self::DefinitionIdentity(error) => error.fmt(formatter),
            Self::DefinitionHash(error) => error.fmt(formatter),
            Self::StaticStorageSemantics(error) => error.fmt(formatter),
            Self::SymbolRequest(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for LirFoundationBuildError {}

fn sort_unique<T, I: PersistentId>(
    mut records: Vec<T>,
    table: LirFoundationTable,
    id_of: impl Fn(&T) -> I,
) -> Result<Vec<T>, LirFoundationBuildError> {
    records.sort_by_key(|record| id_of(record));
    if let Some(pair) = records
        .windows(2)
        .find(|pair| id_of(&pair[0]) == id_of(&pair[1]))
    {
        return Err(LirFoundationBuildError::DuplicateIdentity {
            table,
            identity: *id_of(&pair[0]).as_array(),
        });
    }
    Ok(records)
}

fn sort_mapping<T, I: PersistentId>(
    records: Vec<T>,
    table: LirFoundationTable,
    id_of: impl Fn(&T) -> I,
    derived_of: impl Fn(&T) -> u64,
) -> Result<Vec<T>, LirFoundationBuildError> {
    let records = sort_unique(records, table, &id_of)?;
    let mut derived = BTreeSet::new();
    for record in &records {
        let value = derived_of(record);
        if !derived.insert(value) {
            return Err(LirFoundationBuildError::DuplicateDerivedId {
                table,
                derived: value,
            });
        }
    }
    Ok(records)
}

fn stable_callable_body_order(
    records: Vec<CallableBodyRecord>,
) -> Result<Vec<CallableBodyRecord>, LirFoundationBuildError> {
    struct Node {
        record: CallableBodyRecord,
        dependencies: Vec<[u8; 32]>,
    }

    let mut nodes = BTreeMap::new();
    for record in records {
        let id = *record.id().as_array();
        let key = decode_runtime::<DecodedCallableBodyKey>(record.key_bytes())
            .map_err(LirFoundationBuildError::CallableBodyKey)?;
        let dependencies = match key.kind() {
            DecodedCallableBodyKeyKind::RootGateway { main, .. } => vec![*main.as_array()],
            _ => Vec::new(),
        };
        if nodes
            .insert(
                id,
                Node {
                    record,
                    dependencies,
                },
            )
            .is_some()
        {
            return Err(LirFoundationBuildError::DuplicateIdentity {
                table: LirFoundationTable::CallableBody,
                identity: id,
            });
        }
    }

    let mut indegrees = nodes
        .keys()
        .copied()
        .map(|id| (id, 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<[u8; 32], Vec<[u8; 32]>>::new();
    for (&id, node) in &nodes {
        for dependency in node.dependencies.iter().copied().collect::<BTreeSet<_>>() {
            if !nodes.contains_key(&dependency) {
                return Err(LirFoundationBuildError::MissingCallableBodyDependency {
                    identity: id,
                    dependency,
                });
            }
            let Some(indegree) = indegrees.get_mut(&id) else {
                return Err(LirFoundationBuildError::InconsistentCallableBodyGraph {
                    identity: id,
                });
            };
            *indegree += 1;
            dependents.entry(dependency).or_default().push(id);
        }
    }

    let mut ready = indegrees
        .iter()
        .filter_map(|(&id, &indegree)| (indegree == 0).then_some(id))
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(nodes.len());
    while let Some(id) = ready.pop_first() {
        let Some(node) = nodes.remove(&id) else {
            return Err(LirFoundationBuildError::InconsistentCallableBodyGraph { identity: id });
        };
        ordered.push(node.record);
        if let Some(current_dependents) = dependents.get(&id) {
            for dependent in current_dependents {
                let Some(indegree) = indegrees.get_mut(dependent) else {
                    return Err(LirFoundationBuildError::InconsistentCallableBodyGraph {
                        identity: *dependent,
                    });
                };
                let Some(next_indegree) = indegree.checked_sub(1) else {
                    return Err(LirFoundationBuildError::InconsistentCallableBodyGraph {
                        identity: *dependent,
                    });
                };
                *indegree = next_indegree;
                if *indegree == 0 {
                    ready.insert(*dependent);
                }
            }
        }
    }
    if let Some((&first, _)) = nodes.first_key_value() {
        return Err(LirFoundationBuildError::CallableBodyCycle { first });
    }
    Ok(ordered)
}

fn encode_table_field<T: WireEncode>(
    encoder: &mut Encoder,
    field: u32,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

struct HexIdentity<'a>(&'a [u8; 32]);

impl fmt::Display for HexIdentity<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
