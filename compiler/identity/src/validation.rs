//! Atomic validation for persistent identity graphs decoded from artifacts.

use std::any::{Any, TypeId};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

use scoop_wire::{Digest256, HashError};

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
    dependencies: BTreeSet<IdentityNode>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct CanonicalKeySlot {
    id_type: TypeId,
    key_type: TypeId,
    bytes: [u8; 32],
}

impl CanonicalKeySlot {
    fn new<I: 'static, K: 'static>(bytes: [u8; 32]) -> Self {
        Self {
            id_type: TypeId::of::<I>(),
            key_type: TypeId::of::<K>(),
            bytes,
        }
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
pub struct PendingIdentityValidation {
    candidates: BTreeMap<IdentityNode, Candidate>,
    canonical_keys: HashMap<CanonicalKeySlot, Box<dyn Any>>,
    phase: ValidationPhase,
}

impl Default for PendingIdentityValidation {
    fn default() -> Self {
        Self::new()
    }
}

impl PendingIdentityValidation {
    pub fn new() -> Self {
        Self {
            candidates: BTreeMap::new(),
            canonical_keys: HashMap::new(),
            phase: ValidationPhase::Registering,
        }
    }

    /// Adds an identity that was established by a trusted authority rather
    /// than declared by one of the artifact's delta tables.
    pub fn register_authority<I: PersistentId>(
        &mut self,
        id: I,
    ) -> Result<(), IdentityValidationError> {
        self.require_registration_phase()?;
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
                dependencies: BTreeSet::new(),
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
            };
            record.clone().resolve(|key| {
                key.resolve_identity_key(&mut resolver)
                    .map_err(IdentityKeyResolutionError)
            })
        };

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
            };
            record.clone().resolve(&mut resolver)
        };
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
            };
            decoded.resolve(&mut resolver)
        };
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
        if let Some(node) = find_cycle(&self.candidates) {
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
                dependencies: BTreeSet::new(),
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
        K: 'static,
    {
        let slot = CanonicalKeySlot::new::<I, K>(node.bytes);
        if self.canonical_keys.insert(slot, Box::new(key)).is_some() {
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
}

#[doc(hidden)]
pub struct PendingIdentityResolver<'validation> {
    current: IdentityNode,
    candidates: &'validation mut BTreeMap<IdentityNode, Candidate>,
    canonical_keys: &'validation HashMap<CanonicalKeySlot, Box<dyn Any>>,
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
        self.candidates
            .get_mut(&self.current)
            .expect("current identity disappeared")
            .dependencies
            .insert(target);
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
            .and_then(|key| key.downcast_ref::<K>())
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
    canonical_keys: HashMap<CanonicalKeySlot, Box<dyn Any>>,
}

impl ValidatedIdentityGraph {
    pub fn identity_count(&self) -> usize {
        self.candidates.len()
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
                .and_then(|key| key.downcast_ref::<K>())
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
                .and_then(|key| key.downcast_ref::<K>())
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
            .and_then(|key| key.downcast_ref::<K>())
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
        }
    }
}

impl std::error::Error for IdentityValidationError {}

#[derive(Debug)]
struct IdentityKeyResolutionError(String);

impl fmt::Display for IdentityKeyResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for IdentityKeyResolutionError {}

fn find_cycle(candidates: &BTreeMap<IdentityNode, Candidate>) -> Option<IdentityNode> {
    fn visit(
        node: IdentityNode,
        candidates: &BTreeMap<IdentityNode, Candidate>,
        visiting: &mut BTreeSet<IdentityNode>,
        visited: &mut BTreeSet<IdentityNode>,
    ) -> Option<IdentityNode> {
        if visited.contains(&node) {
            return None;
        }
        if !visiting.insert(node) {
            return Some(node);
        }
        let candidate = candidates.get(&node)?;
        for dependency in &candidate.dependencies {
            if let Some(cycle) = visit(*dependency, candidates, visiting, visited) {
                return Some(cycle);
            }
        }
        visiting.remove(&node);
        visited.insert(node);
        None
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for node in candidates.keys() {
        if let Some(cycle) = visit(*node, candidates, &mut visiting, &mut visited) {
            return Some(cycle);
        }
    }
    None
}

fn write_hex(bytes: &[u8; 32], formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
