//! Atomic validation for persistent identity graphs decoded from artifacts.

use std::any::{Any, TypeId};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::Arc;

use scoop_wire::budget::{GRAPH_EDGE_BYTES, READY_SET_ELEMENT_BYTES};
use scoop_wire::{BudgetMeter, Digest256, HashError, WireError, WireErrorKind, WirePath};

use crate::ids::PersistentIdConstruction;
use crate::{
    CallableBodyKey, CborIdentityKey, CborIdentityRecord, DecodedCallableBodyKey,
    DecodedCborIdentityRecord, DecodedPersistentId, DecodedRuntimeIdentityRecord,
    DecodedSourceNativeExternalContractRecord, PersistentCallableBodyId, PersistentId,
    PersistentIdResolver, PersistentKeyResolver, PersistentSourceNativeExternalContractId,
    RuntimeIdentityKey, RuntimeIdentityRecord, RuntimeIdentityRecordValidationError,
    SourceNativeExternalContractKey,
};

mod decoded;
pub use decoded::DecodedIdentityKey;

mod semantic;
pub use semantic::{
    HirIdentityLayer, ImportedIdentityId, ImportedIdentityLayer, ImportedIdentityLayers,
    ImportedIdentityMap, LirIdentityLayer, MirIdentityLayer, SemanticIdentityImportError,
    SemanticIdentitySession, SemanticOriginFingerprint,
};

/// Artifact layer that first introduces a persistent identity record.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum IdentityLayer {
    Hir,
    Mir,
    Lir,
}

impl fmt::Display for IdentityLayer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Hir => "HIR",
            Self::Mir => "MIR",
            Self::Lir => "LIR",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct IdentityNode {
    kind: &'static str,
    bytes: [u8; 32],
}

impl IdentityNode {
    fn decoded<I: PersistentId>(id: DecodedPersistentId<I>) -> Self {
        Self {
            kind: I::KIND,
            bytes: *id.as_array(),
        }
    }

    fn trusted<I: PersistentId>(id: I) -> Self {
        Self {
            kind: I::KIND,
            bytes: *id.as_array(),
        }
    }
}

#[derive(Debug)]
struct Candidate {
    layer: Option<IdentityLayer>,
    resolved: bool,
    dependency_count: u64,
    dependents: Vec<IdentityNode>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct CanonicalKeySlot {
    kind: &'static str,
    id_type: TypeId,
    key_type: TypeId,
    bytes: [u8; 32],
}

impl CanonicalKeySlot {
    fn new<I: PersistentId + 'static, K: 'static>(bytes: [u8; 32]) -> Self {
        Self {
            kind: I::KIND,
            id_type: TypeId::of::<I>(),
            key_type: TypeId::of::<K>(),
            bytes,
        }
    }
}

trait ErasedCanonicalKey: Any {
    fn as_any(&self) -> &dyn Any;

    fn equals(&self, other: &dyn ErasedCanonicalKey) -> bool;
}

impl<T> ErasedCanonicalKey for T
where
    T: Any + Eq,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn equals(&self, other: &dyn ErasedCanonicalKey) -> bool {
        other.as_any().downcast_ref::<Self>() == Some(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ValidationPhase {
    Registering,
    Resolving,
    Poisoned,
}

/// Pending all-layer identity transaction.
///
/// Callers first register every HIR/MIR/LIR identity record, then resolve each
/// registered record. No trusted id or canonical record is exposed until
/// [`Self::finish`] succeeds.
pub struct PendingIdentityValidation<'meter> {
    candidates: BTreeMap<IdentityNode, Candidate>,
    canonical_keys: HashMap<CanonicalKeySlot, Arc<dyn ErasedCanonicalKey>>,
    phase: ValidationPhase,
    meter: Option<&'meter mut BudgetMeter>,
    resource_error: Option<WireError>,
    resource_path: WirePath,
}

impl Default for PendingIdentityValidation<'static> {
    fn default() -> Self {
        Self::new()
    }
}

impl PendingIdentityValidation<'static> {
    pub fn new() -> Self {
        Self {
            candidates: BTreeMap::new(),
            canonical_keys: HashMap::new(),
            phase: ValidationPhase::Registering,
            meter: None,
            resource_error: None,
            resource_path: WirePath::default(),
        }
    }
}

impl<'meter> PendingIdentityValidation<'meter> {
    /// Starts an artifact-reader transaction backed by the artifact's shared
    /// resource meter.
    pub fn with_meter(meter: &'meter mut BudgetMeter) -> Self {
        Self {
            candidates: BTreeMap::new(),
            canonical_keys: HashMap::new(),
            phase: ValidationPhase::Registering,
            meter: Some(meter),
            resource_error: None,
            resource_path: WirePath::default(),
        }
    }

    /// Adds an identity that was established by a trusted authority rather
    /// than declared by one of the artifact's delta tables.
    pub fn register_authority<I: PersistentId>(
        &mut self,
        id: I,
    ) -> Result<(), IdentityValidationError> {
        self.require_registration_phase()?;
        self.insert_resolved_leaf(id)
    }

    /// Adds a hash identity whose canonical preimage was verified by its
    /// owning decoded record before graph registration.
    ///
    /// The caller must subsequently supply the resolved preimage through
    /// [`Self::resolve_verified_leaf`]. Keeping these fingerprints in the
    /// ordinary layer delta makes them participate in typed remap and
    /// semantic-key conflict detection just like every other identity kind.
    pub fn register_verified_leaf<I: PersistentId>(
        &mut self,
        layer: IdentityLayer,
        id: I,
    ) -> Result<(), IdentityValidationError> {
        self.require_registration_phase()?;
        self.insert_candidate(layer, IdentityNode::trusted(id))
    }

    fn insert_resolved_leaf<I: PersistentId>(
        &mut self,
        id: I,
    ) -> Result<(), IdentityValidationError> {
        let node = IdentityNode::trusted(id);
        if self.candidates.contains_key(&node) {
            return self.fail(IdentityValidationError::DuplicateIdentity {
                kind: node.kind,
                id: node.bytes,
            });
        }
        self.candidates.insert(
            node,
            Candidate {
                layer: None,
                resolved: true,
                dependency_count: 0,
                dependents: Vec::new(),
            },
        );
        Ok(())
    }

    /// Registers one decoded record and verifies its raw hash preimage.
    pub fn register<I, D>(
        &mut self,
        layer: IdentityLayer,
        record: &DecodedCborIdentityRecord<I, D>,
    ) -> Result<(), IdentityValidationError>
    where
        I: PersistentId + 'static,
        D: DecodedIdentityKey<I>,
    {
        self.require_registration_phase()?;
        let expected = match record.key().candidate_identity() {
            Ok(expected) => expected,
            Err(error) => {
                return self.fail(IdentityValidationError::Hash {
                    kind: I::KIND,
                    error,
                });
            }
        };
        if let Err(mismatch) = record.decoded_id().verify(expected) {
            return self.fail(IdentityValidationError::IdentityMismatch {
                kind: I::KIND,
                expected: *mismatch.expected().as_array(),
                actual: *mismatch.actual(),
            });
        }

        self.insert_candidate(layer, IdentityNode::decoded(record.decoded_id()))
    }

    pub fn register_source_native_contract(
        &mut self,
        layer: IdentityLayer,
        record: &DecodedSourceNativeExternalContractRecord,
    ) -> Result<(), IdentityValidationError> {
        self.require_registration_phase()?;
        let expected = record.candidate_identity().map_err(|error| {
            self.phase = ValidationPhase::Poisoned;
            IdentityValidationError::Hash {
                kind: PersistentSourceNativeExternalContractId::KIND,
                error,
            }
        })?;
        let decoded = record.decoded_id();
        if let Err(mismatch) = decoded.verify(expected) {
            return self.fail(IdentityValidationError::IdentityMismatch {
                kind: PersistentSourceNativeExternalContractId::KIND,
                expected: *mismatch.expected().as_array(),
                actual: *mismatch.actual(),
            });
        }
        self.insert_candidate(layer, IdentityNode::decoded(decoded))
    }

    pub fn register_callable_body(
        &mut self,
        layer: IdentityLayer,
        record: &DecodedRuntimeIdentityRecord<PersistentCallableBodyId>,
    ) -> Result<(), IdentityValidationError> {
        self.require_registration_phase()?;
        let decoded = record.decoded_id();
        if let Err(error) = record.clone().validate_key::<DecodedCallableBodyKey>() {
            return match error {
                RuntimeIdentityRecordValidationError::Id(mismatch) => {
                    self.fail(IdentityValidationError::IdentityMismatch {
                        kind: PersistentCallableBodyId::KIND,
                        expected: *mismatch.expected().as_array(),
                        actual: *mismatch.actual(),
                    })
                }
                error => self.fail(IdentityValidationError::InvalidRecord {
                    kind: PersistentCallableBodyId::KIND,
                    id: *decoded.as_array(),
                    reason: error.to_string(),
                }),
            };
        }
        self.insert_candidate(layer, IdentityNode::decoded(decoded))
    }

    /// Resolves and rehashes one previously registered record.
    pub fn resolve<I, D>(
        &mut self,
        record: &DecodedCborIdentityRecord<I, D>,
    ) -> Result<(), IdentityValidationError>
    where
        I: PersistentId + 'static,
        D: DecodedIdentityKey<I>,
        <D::Canonical as CborIdentityKey<I>>::Error: fmt::Display,
    {
        let node = IdentityNode::decoded(record.decoded_id());
        self.start_resolution(node)?;

        let resolved = {
            let mut resolver = PendingIdentityResolver {
                current: node,
                candidates: &mut self.candidates,
                canonical_keys: &self.canonical_keys,
                meter: self.meter.as_deref_mut(),
                resource_error: &mut self.resource_error,
                resource_path: &self.resource_path,
            };
            record.clone().resolve(|key| {
                key.resolve_identity_key(&mut resolver)
                    .map_err(IdentityKeyResolutionError)
            })
        };
        self.require_no_resource_error()?;

        let resolved = match resolved {
            Ok(resolved) => resolved,
            Err(error) => {
                return self.fail(IdentityValidationError::InvalidRecord {
                    kind: node.kind,
                    id: node.bytes,
                    reason: error.to_string(),
                });
            }
        };
        self.store_resolved_key::<I, D::Canonical>(node, resolved.key().clone())
    }

    pub fn resolve_source_native_contract(
        &mut self,
        record: &DecodedSourceNativeExternalContractRecord,
    ) -> Result<(), IdentityValidationError> {
        let node = IdentityNode::decoded(record.decoded_id());
        self.start_resolution(node)?;
        let resolved = {
            let mut resolver = PendingIdentityResolver {
                current: node,
                candidates: &mut self.candidates,
                canonical_keys: &self.canonical_keys,
                meter: self.meter.as_deref_mut(),
                resource_error: &mut self.resource_error,
                resource_path: &self.resource_path,
            };
            record.clone().resolve(&mut resolver)
        };
        self.require_no_resource_error()?;
        let resolved = match resolved {
            Ok(resolved) => resolved,
            Err(error) => {
                return self.fail(IdentityValidationError::InvalidRecord {
                    kind: node.kind,
                    id: node.bytes,
                    reason: error.to_string(),
                });
            }
        };
        self.store_resolved_key::<
            PersistentSourceNativeExternalContractId,
            SourceNativeExternalContractKey,
        >(node, resolved.key())
    }

    pub fn resolve_callable_body(
        &mut self,
        record: &DecodedRuntimeIdentityRecord<PersistentCallableBodyId>,
    ) -> Result<(), IdentityValidationError> {
        let node = IdentityNode::decoded(record.decoded_id());
        self.start_resolution(node)?;
        let decoded = match record.decode_key::<DecodedCallableBodyKey>() {
            Ok(decoded) => decoded,
            Err(error) => {
                return self.fail(IdentityValidationError::InvalidRecord {
                    kind: node.kind,
                    id: node.bytes,
                    reason: error.to_string(),
                });
            }
        };
        let resolved = {
            let mut resolver = PendingIdentityResolver {
                current: node,
                candidates: &mut self.candidates,
                canonical_keys: &self.canonical_keys,
                meter: self.meter.as_deref_mut(),
                resource_error: &mut self.resource_error,
                resource_path: &self.resource_path,
            };
            decoded.resolve(&mut resolver)
        };
        self.require_no_resource_error()?;
        let resolved = match resolved {
            Ok(resolved) => resolved,
            Err(error) => {
                return self.fail(IdentityValidationError::InvalidRecord {
                    kind: node.kind,
                    id: node.bytes,
                    reason: error.to_string(),
                });
            }
        };
        let rebuilt = match RuntimeIdentityRecord::from_key(&resolved) {
            Ok(rebuilt) => rebuilt,
            Err(error) => {
                return self.fail(IdentityValidationError::InvalidRecord {
                    kind: node.kind,
                    id: node.bytes,
                    reason: error.to_string(),
                });
            }
        };
        if let Err(mismatch) = record.decoded_id().verify(rebuilt.id()) {
            return self.fail(IdentityValidationError::IdentityMismatch {
                kind: node.kind,
                expected: *mismatch.expected().as_array(),
                actual: *mismatch.actual(),
            });
        }
        self.store_resolved_key::<PersistentCallableBodyId, CallableBodyKey>(node, resolved)
    }

    /// Resolves the canonical preimage of a previously verified hash leaf.
    /// References observed by the callback are added to the same dependency
    /// graph as ordinary identity records.
    pub fn resolve_verified_leaf<I, K, E>(
        &mut self,
        id: I,
        resolve: impl FnOnce(&mut PendingIdentityResolver<'_>) -> Result<K, E>,
    ) -> Result<(), IdentityValidationError>
    where
        I: PersistentId + 'static,
        K: Eq + 'static,
        E: fmt::Display,
    {
        let node = IdentityNode::trusted(id);
        self.start_resolution(node)?;
        let resolved = {
            let mut resolver = PendingIdentityResolver {
                current: node,
                candidates: &mut self.candidates,
                canonical_keys: &self.canonical_keys,
                meter: self.meter.as_deref_mut(),
                resource_error: &mut self.resource_error,
                resource_path: &self.resource_path,
            };
            resolve(&mut resolver)
        };
        self.require_no_resource_error()?;
        match resolved {
            Ok(key) => self.store_resolved_key::<I, K>(node, key),
            Err(error) => self.fail(IdentityValidationError::InvalidRecord {
                kind: node.kind,
                id: node.bytes,
                reason: error.to_string(),
            }),
        }
    }

    /// Completes validation only if every declared record was resolved and the
    /// union dependency graph is acyclic.
    pub fn finish(mut self) -> Result<ValidatedIdentityGraph, IdentityValidationError> {
        if self.phase == ValidationPhase::Poisoned {
            return Err(IdentityValidationError::Poisoned);
        }
        if let Some((node, _)) = self
            .candidates
            .iter()
            .find(|(_, candidate)| candidate.layer.is_some() && !candidate.resolved)
        {
            return Err(IdentityValidationError::UnresolvedIdentity {
                kind: node.kind,
                id: node.bytes,
            });
        }
        let node_count = u64::try_from(self.candidates.len())
            .map_err(|_| resource_error(WireErrorKind::IntegerOutOfRange, &self.resource_path))?;
        let edge_count = self
            .candidates
            .values()
            .try_fold(0_u64, |count, candidate| {
                count
                    .checked_add(candidate.dependency_count)
                    .ok_or_else(|| {
                        resource_error(WireErrorKind::IntegerOutOfRange, &self.resource_path)
                    })
            })?;
        if let Some(meter) = self.meter.as_deref_mut() {
            meter
                .charge_stable_kahn(node_count, edge_count, &self.resource_path)
                .map_err(IdentityValidationError::Resource)?;
        }
        if let Some(node) = find_cycle(&mut self.candidates, node_count, &self.resource_path)? {
            return Err(IdentityValidationError::DependencyCycle {
                kind: node.kind,
                id: node.bytes,
            });
        }
        self.canonical_keys.shrink_to_fit();
        Ok(ValidatedIdentityGraph {
            candidates: self.candidates,
            canonical_keys: self.canonical_keys,
        })
    }

    fn require_registration_phase(&mut self) -> Result<(), IdentityValidationError> {
        match self.phase {
            ValidationPhase::Registering => Ok(()),
            ValidationPhase::Resolving => self.fail(IdentityValidationError::RegistrationClosed),
            ValidationPhase::Poisoned => Err(IdentityValidationError::Poisoned),
        }
    }

    fn insert_candidate(
        &mut self,
        layer: IdentityLayer,
        node: IdentityNode,
    ) -> Result<(), IdentityValidationError> {
        if self.candidates.contains_key(&node) {
            return self.fail(IdentityValidationError::DuplicateIdentity {
                kind: node.kind,
                id: node.bytes,
            });
        }
        self.candidates.insert(
            node,
            Candidate {
                layer: Some(layer),
                resolved: false,
                dependency_count: 0,
                dependents: Vec::new(),
            },
        );
        Ok(())
    }

    fn start_resolution(&mut self, node: IdentityNode) -> Result<(), IdentityValidationError> {
        if self.phase == ValidationPhase::Poisoned {
            return Err(IdentityValidationError::Poisoned);
        }
        self.phase = ValidationPhase::Resolving;
        match self.candidates.get(&node) {
            Some(candidate) if candidate.layer.is_some() && !candidate.resolved => Ok(()),
            Some(_) => self.fail(IdentityValidationError::AlreadyResolved {
                kind: node.kind,
                id: node.bytes,
            }),
            None => self.fail(IdentityValidationError::UnregisteredIdentity {
                kind: node.kind,
                id: node.bytes,
            }),
        }
    }

    fn store_resolved_key<I, K>(
        &mut self,
        node: IdentityNode,
        key: K,
    ) -> Result<(), IdentityValidationError>
    where
        I: PersistentId + 'static,
        K: Eq + 'static,
    {
        let slot = CanonicalKeySlot::new::<I, K>(node.bytes);
        if self.canonical_keys.insert(slot, Arc::new(key)).is_some() {
            return self.fail(IdentityValidationError::AlreadyResolved {
                kind: node.kind,
                id: node.bytes,
            });
        }
        self.candidates
            .get_mut(&node)
            .expect("registered identity disappeared")
            .resolved = true;
        Ok(())
    }

    fn fail<T>(&mut self, error: IdentityValidationError) -> Result<T, IdentityValidationError> {
        self.phase = ValidationPhase::Poisoned;
        Err(error)
    }

    fn require_no_resource_error(&mut self) -> Result<(), IdentityValidationError> {
        match self.resource_error.take() {
            Some(error) => self.fail(IdentityValidationError::Resource(error)),
            None => Ok(()),
        }
    }
}

#[doc(hidden)]
pub struct PendingIdentityResolver<'validation> {
    current: IdentityNode,
    candidates: &'validation mut BTreeMap<IdentityNode, Candidate>,
    canonical_keys: &'validation HashMap<CanonicalKeySlot, Arc<dyn ErasedCanonicalKey>>,
    meter: Option<&'validation mut BudgetMeter>,
    resource_error: &'validation mut Option<WireError>,
    resource_path: &'validation WirePath,
}

impl<I> PersistentIdResolver<I> for PendingIdentityResolver<'_>
where
    I: PersistentId + PersistentIdConstruction,
{
    type Error = IdentityReferenceError;

    fn resolve(&mut self, id: DecodedPersistentId<I>) -> Result<I, Self::Error> {
        let target = IdentityNode::decoded(id);
        let target_layer = self
            .candidates
            .get(&target)
            .ok_or(IdentityReferenceError::Missing {
                kind: target.kind,
                id: target.bytes,
            })?
            .layer;
        let source_layer = self
            .candidates
            .get(&self.current)
            .expect("current identity disappeared")
            .layer
            .expect("authority cannot own an identity record");
        if target_layer.is_some_and(|layer| layer > source_layer) {
            return Err(IdentityReferenceError::FutureLayer {
                source: source_layer,
                target: target_layer.expect("checked as present"),
                kind: target.kind,
                id: target.bytes,
            });
        }
        let next_dependency_count = match self
            .candidates
            .get(&self.current)
            .expect("current identity disappeared")
            .dependency_count
            .checked_add(1)
        {
            Some(count) => count,
            None => {
                *self.resource_error = Some(WireError::new(
                    WireErrorKind::IntegerOutOfRange,
                    self.resource_path.clone(),
                    None,
                ));
                return Err(IdentityReferenceError::ResourceLimit);
            }
        };
        if let Some(meter) = self.meter.as_deref_mut()
            && let Err(error) = meter.charge_edges(1, self.resource_path)
        {
            *self.resource_error = Some(error);
            return Err(IdentityReferenceError::ResourceLimit);
        }
        let target_candidate = self
            .candidates
            .get_mut(&target)
            .expect("resolved identity target disappeared");
        if target_candidate.dependents.try_reserve_exact(1).is_err() {
            *self.resource_error = Some(WireError::new(
                WireErrorKind::ResourceAllocation {
                    requested_logical_bytes: GRAPH_EDGE_BYTES,
                    requested_slots: 1,
                },
                self.resource_path.clone(),
                None,
            ));
            return Err(IdentityReferenceError::ResourceLimit);
        }
        target_candidate.dependents.push(self.current);
        let source_candidate = self
            .candidates
            .get_mut(&self.current)
            .expect("current identity disappeared");
        source_candidate.dependency_count = next_dependency_count;
        Ok(I::from_digest(Digest256::from_array(target.bytes)))
    }
}

impl<I, K> PersistentKeyResolver<I, K> for PendingIdentityResolver<'_>
where
    I: PersistentId + PersistentIdConstruction + 'static,
    K: Clone + 'static,
{
    type Error = IdentityReferenceError;

    fn resolve_key(&mut self, id: DecodedPersistentId<I>) -> Result<K, Self::Error> {
        let resolved = <Self as PersistentIdResolver<I>>::resolve(self, id)?;
        let slot = CanonicalKeySlot::new::<I, K>(*resolved.as_array());
        self.canonical_keys
            .get(&slot)
            .and_then(|key| key.as_any().downcast_ref::<K>())
            .cloned()
            .ok_or(IdentityReferenceError::KeyUnavailable {
                kind: I::KIND,
                id: *resolved.as_array(),
            })
    }
}

/// Fully validated persistent identity graph.
///
/// Constructing this type proves that every declared record was rehashed,
/// every reference resolved to the same or an earlier layer, and the complete
/// dependency graph is acyclic.
pub struct ValidatedIdentityGraph {
    candidates: BTreeMap<IdentityNode, Candidate>,
    canonical_keys: HashMap<CanonicalKeySlot, Arc<dyn ErasedCanonicalKey>>,
}

impl ValidatedIdentityGraph {
    pub fn identity_count(&self) -> usize {
        self.candidates.len()
    }

    /// Number of artifact-declared identities that require a session-local
    /// remap entry. Trusted authority leaves are excluded.
    pub fn declared_identity_count(&self) -> usize {
        self.candidates
            .values()
            .filter(|candidate| candidate.layer.is_some())
            .count()
    }

    /// Reconstructs the canonical records introduced by one layer and key
    /// family, sorted by raw persistent id.
    pub fn records<I, K>(
        &self,
        layer: IdentityLayer,
    ) -> Result<Vec<CborIdentityRecord<I, K>>, IdentityValidationError>
    where
        I: PersistentId + 'static,
        K: CborIdentityKey<I> + Clone + 'static,
        K::Error: fmt::Display,
    {
        let mut records = Vec::new();
        for (node, candidate) in &self.candidates {
            if node.kind != I::KIND || candidate.layer != Some(layer) {
                continue;
            }
            let slot = CanonicalKeySlot::new::<I, K>(node.bytes);
            let Some(key) = self
                .canonical_keys
                .get(&slot)
                .and_then(|key| key.as_any().downcast_ref::<K>())
            else {
                continue;
            };
            let record = CborIdentityRecord::from_key(key.clone()).map_err(|error| {
                IdentityValidationError::InvalidRecord {
                    kind: I::KIND,
                    id: node.bytes,
                    reason: error.to_string(),
                }
            })?;
            if record.id().as_array() != &node.bytes {
                return Err(IdentityValidationError::IdentityMismatch {
                    kind: I::KIND,
                    expected: *record.id().as_array(),
                    actual: node.bytes,
                });
            }
            records.push(record);
        }
        Ok(records)
    }

    /// Reconstructs runtime-encoded records introduced by one layer and key
    /// family, sorted by raw persistent id.
    pub fn runtime_records<I, K>(
        &self,
        layer: IdentityLayer,
    ) -> Result<Vec<RuntimeIdentityRecord<I>>, IdentityValidationError>
    where
        I: PersistentId + 'static,
        K: RuntimeIdentityKey<I> + Clone + 'static,
        K::Error: fmt::Display,
    {
        let mut records = Vec::new();
        for (node, candidate) in &self.candidates {
            if node.kind != I::KIND || candidate.layer != Some(layer) {
                continue;
            }
            let slot = CanonicalKeySlot::new::<I, K>(node.bytes);
            let Some(key) = self
                .canonical_keys
                .get(&slot)
                .and_then(|key| key.as_any().downcast_ref::<K>())
            else {
                continue;
            };
            let record = RuntimeIdentityRecord::from_key(key).map_err(|error| {
                IdentityValidationError::InvalidRecord {
                    kind: I::KIND,
                    id: node.bytes,
                    reason: error.to_string(),
                }
            })?;
            if record.id().as_array() != &node.bytes {
                return Err(IdentityValidationError::IdentityMismatch {
                    kind: I::KIND,
                    expected: *record.id().as_array(),
                    actual: node.bytes,
                });
            }
            records.push(record);
        }
        Ok(records)
    }
}

impl<I> PersistentIdResolver<I> for ValidatedIdentityGraph
where
    I: PersistentId + PersistentIdConstruction,
{
    type Error = IdentityReferenceError;

    fn resolve(&mut self, id: DecodedPersistentId<I>) -> Result<I, Self::Error> {
        let node = IdentityNode::decoded(id);
        self.candidates
            .get(&node)
            .filter(|candidate| candidate.resolved)
            .ok_or(IdentityReferenceError::Missing {
                kind: node.kind,
                id: node.bytes,
            })?;
        Ok(I::from_digest(Digest256::from_array(node.bytes)))
    }
}

impl<I, K> PersistentKeyResolver<I, K> for ValidatedIdentityGraph
where
    I: PersistentId + PersistentIdConstruction + 'static,
    K: Clone + 'static,
{
    type Error = IdentityReferenceError;

    fn resolve_key(&mut self, id: DecodedPersistentId<I>) -> Result<K, Self::Error> {
        let resolved = <Self as PersistentIdResolver<I>>::resolve(self, id)?;
        let slot = CanonicalKeySlot::new::<I, K>(*resolved.as_array());
        self.canonical_keys
            .get(&slot)
            .and_then(|key| key.as_any().downcast_ref::<K>())
            .cloned()
            .ok_or(IdentityReferenceError::KeyUnavailable {
                kind: I::KIND,
                id: *resolved.as_array(),
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityReferenceError {
    Missing {
        kind: &'static str,
        id: [u8; 32],
    },
    FutureLayer {
        source: IdentityLayer,
        target: IdentityLayer,
        kind: &'static str,
        id: [u8; 32],
    },
    KeyUnavailable {
        kind: &'static str,
        id: [u8; 32],
    },
    #[doc(hidden)]
    ResourceLimit,
}

impl fmt::Display for IdentityReferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { kind, id } => {
                write!(formatter, "missing {kind} identity ")?;
                write_hex(id, formatter)
            }
            Self::FutureLayer {
                source,
                target,
                kind,
                id,
            } => {
                write!(
                    formatter,
                    "{source} identity references future {target} {kind} identity "
                )?;
                write_hex(id, formatter)
            }
            Self::KeyUnavailable { kind, id } => {
                write!(formatter, "canonical key for {kind} identity ")?;
                write_hex(id, formatter)?;
                formatter.write_str(" is not available in dependency-first order")
            }
            Self::ResourceLimit => formatter.write_str("identity resource limit exceeded"),
        }
    }
}

impl std::error::Error for IdentityReferenceError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityValidationError {
    RegistrationClosed,
    Poisoned,
    Hash {
        kind: &'static str,
        error: HashError,
    },
    IdentityMismatch {
        kind: &'static str,
        expected: [u8; 32],
        actual: [u8; 32],
    },
    DuplicateIdentity {
        kind: &'static str,
        id: [u8; 32],
    },
    UnregisteredIdentity {
        kind: &'static str,
        id: [u8; 32],
    },
    AlreadyResolved {
        kind: &'static str,
        id: [u8; 32],
    },
    InvalidRecord {
        kind: &'static str,
        id: [u8; 32],
        reason: String,
    },
    UnresolvedIdentity {
        kind: &'static str,
        id: [u8; 32],
    },
    DependencyCycle {
        kind: &'static str,
        id: [u8; 32],
    },
    Resource(WireError),
}

impl fmt::Display for IdentityValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RegistrationClosed => formatter.write_str("identity registration is closed"),
            Self::Poisoned => formatter.write_str("identity validation transaction has failed"),
            Self::Hash { kind, error } => {
                write!(formatter, "failed to hash {kind} identity: {error}")
            }
            Self::IdentityMismatch {
                kind,
                expected,
                actual,
            } => {
                write!(formatter, "{kind} identity mismatch: expected ")?;
                write_hex(expected, formatter)?;
                formatter.write_str(", found ")?;
                write_hex(actual, formatter)
            }
            Self::DuplicateIdentity { kind, id } => {
                write!(formatter, "duplicate {kind} identity ")?;
                write_hex(id, formatter)
            }
            Self::UnregisteredIdentity { kind, id } => {
                write!(formatter, "unregistered {kind} identity ")?;
                write_hex(id, formatter)
            }
            Self::AlreadyResolved { kind, id } => {
                write!(formatter, "{kind} identity was resolved more than once: ")?;
                write_hex(id, formatter)
            }
            Self::InvalidRecord { kind, id, reason } => {
                write!(formatter, "invalid {kind} identity record ")?;
                write_hex(id, formatter)?;
                write!(formatter, ": {reason}")
            }
            Self::UnresolvedIdentity { kind, id } => {
                write!(formatter, "unresolved {kind} identity ")?;
                write_hex(id, formatter)
            }
            Self::DependencyCycle { kind, id } => {
                write!(
                    formatter,
                    "identity dependency cycle contains {kind} identity "
                )?;
                write_hex(id, formatter)
            }
            Self::Resource(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for IdentityValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct IdentityKeyResolutionError(String);

impl fmt::Display for IdentityKeyResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for IdentityKeyResolutionError {}

fn find_cycle(
    candidates: &mut BTreeMap<IdentityNode, Candidate>,
    node_count: u64,
    path: &WirePath,
) -> Result<Option<IdentityNode>, IdentityValidationError> {
    let capacity = usize::try_from(node_count)
        .map_err(|_| resource_error(WireErrorKind::IntegerOutOfRange, path))?;
    let mut ready_storage = Vec::new();
    ready_storage.try_reserve_exact(capacity).map_err(|_| {
        IdentityValidationError::Resource(WireError::new(
            WireErrorKind::ResourceAllocation {
                requested_logical_bytes: node_count.saturating_mul(READY_SET_ELEMENT_BYTES),
                requested_slots: node_count,
            },
            path.clone(),
            None,
        ))
    })?;
    let mut ready = BinaryHeap::from(ready_storage);
    for (node, candidate) in candidates.iter() {
        if candidate.dependency_count == 0 {
            ready.push(Reverse(*node));
        }
    }

    let mut completed = 0_u64;
    while let Some(Reverse(node)) = ready.pop() {
        completed += 1;
        let dependents = std::mem::take(
            &mut candidates
                .get_mut(&node)
                .expect("ready identity disappeared")
                .dependents,
        );
        for dependent in dependents {
            let candidate = candidates
                .get_mut(&dependent)
                .expect("identity dependent disappeared");
            candidate.dependency_count -= 1;
            if candidate.dependency_count == 0 {
                ready.push(Reverse(dependent));
            }
        }
    }
    if completed == node_count {
        Ok(None)
    } else {
        Ok(candidates
            .iter()
            .find_map(|(node, candidate)| (candidate.dependency_count != 0).then_some(*node)))
    }
}

fn resource_error(kind: WireErrorKind, path: &WirePath) -> IdentityValidationError {
    IdentityValidationError::Resource(WireError::new(kind, path.clone(), None))
}

fn write_hex(bytes: &[u8; 32], formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod property_tests;
