//! Atomic validation for persistent identity graphs decoded from artifacts.

use std::any::{Any, TypeId};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::fmt;
use std::sync::Arc;

use scoop_wire::{Digest256, HashError, WireError, WireErrorKind, WirePath};

use crate::ids::PersistentIdConstruction;
use crate::{
    CallableBodyKey, CanonicalCAbiLayoutFingerprint, CanonicalCAbiSignatureFingerprint,
    CborIdentityKey, CborIdentityRecord, DecodedCallableBodyKey,
    DecodedCanonicalCAbiLayoutFingerprintRecord, DecodedCanonicalCAbiSignatureFingerprintRecord,
    DecodedCborIdentityRecord, DecodedNativeExternalContractRecord, DecodedPersistentId,
    DecodedRuntimeIdentityRecord, DecodedSourceNativeExternalContractRecord,
    NativeExternalContractFingerprint, NativeExternalContractFingerprintError,
    NativeExternalContractFingerprintInput, PersistentCallableBodyId, PersistentId,
    PersistentIdResolver, PersistentKeyResolver, PersistentSourceNativeExternalContractId,
    RuntimeIdentityKey, RuntimeIdentityRecord, RuntimeIdentityRecordValidationError,
    SourceNativeExternalContractKey,
};

mod canonical;
mod decoded;
mod external;
mod records;
pub use decoded::DecodedIdentityKey;

mod semantic;
pub use semantic::{
    HirIdentityLayer, ImportedIdentityId, ImportedIdentityLayer, ImportedIdentityLayers,
    ImportedIdentityMap, LirIdentityLayer, MirIdentityLayer, SemanticIdentityImport,
    SemanticIdentityImportError, SemanticIdentitySession, SemanticOriginFingerprint,
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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

struct Candidate {
    trusted_id: Arc<dyn Any>,
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

trait ErasedCanonicalKey: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;

    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;

    fn equals(&self, other: &dyn ErasedCanonicalKey) -> bool;
}

impl<T> ErasedCanonicalKey for T
where
    T: Any + Eq + Send + Sync,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
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
pub struct PendingIdentityValidation {
    candidates: HashMap<IdentityNode, Candidate>,
    canonical_keys: HashMap<CanonicalKeySlot, Arc<dyn ErasedCanonicalKey>>,
    phase: ValidationPhase,

    resource_error: Option<WireError>,
    resource_path: WirePath,
}

impl Default for PendingIdentityValidation {
    fn default() -> Self {
        Self::new()
    }
}

impl PendingIdentityValidation {
    pub fn new() -> Self {
        Self {
            candidates: HashMap::new(),
            canonical_keys: HashMap::new(),
            phase: ValidationPhase::Registering,

            resource_error: None,
            resource_path: WirePath::default(),
        }
    }
}

impl PendingIdentityValidation {
    /// Adds an identity that was established by a trusted authority rather
    /// than declared by one of the artifact's delta tables.
    pub fn register_authority<I: PersistentId + 'static>(
        &mut self,
        id: I,
    ) -> Result<(), IdentityValidationError> {
        self.require_registration_phase()?;
        self.insert_resolved_leaf(id)
    }

    /// Imports one complete canonical identity record from an already
    /// validated external authority.
    ///
    /// Unlike [`Self::register_authority`], this retains the canonical key so
    /// identities declared by the current artifact can safely validate
    /// kind-sensitive references to the external entity. The imported record
    /// remains an authority leaf and is therefore excluded from this
    /// artifact's declared identity delta and later session remap.
    pub fn register_external_canonical_authority<I, K>(
        &mut self,
        record: CborIdentityRecord<I, K>,
    ) -> Result<(), IdentityValidationError>
    where
        I: PersistentId + 'static,
        K: CborIdentityKey<I> + Eq + Send + Sync + 'static,
    {
        self.require_registration_phase()?;
        let node = IdentityNode::trusted(record.id());
        if self.candidates.contains_key(&node) {
            return self.fail(IdentityValidationError::DuplicateIdentity {
                kind: node.kind,
                id: node.bytes,
            });
        }

        let slot = CanonicalKeySlot::new::<I, K>(node.bytes);
        if self.canonical_keys.contains_key(&slot) {
            return self.fail(IdentityValidationError::IdentityCollision {
                kind: node.kind,
                id: node.bytes,
            });
        }

        self.reserve_candidate_slot()?;
        self.reserve_canonical_key_slot()?;
        self.candidates.insert(
            node,
            Candidate {
                trusted_id: Arc::new(record.id()),
                layer: None,
                resolved: true,
                dependency_count: 0,
                dependents: Vec::new(),
            },
        );
        self.canonical_keys.insert(slot, record.into_shared_key());
        Ok(())
    }

    /// Registers one typed external reference whose canonical key is owned by
    /// another artifact in the already validated dependency graph.
    ///
    /// The raw digest never escapes this transaction. It may only satisfy
    /// references from records that are themselves rehashed here, and it is
    /// excluded from the semantic import delta because this artifact does not
    /// declare the referenced identity.
    pub fn register_external_source_type(
        &mut self,
        id: DecodedPersistentId<crate::PersistentTypeId>,
    ) -> Result<(), IdentityValidationError> {
        self.register_external_reference(id)
    }

    pub fn register_external_generic_type(
        &mut self,
        id: DecodedPersistentId<crate::PersistentGenericTypeId>,
    ) -> Result<(), IdentityValidationError> {
        self.register_external_reference(id)
    }

    pub fn register_c_abi_signature(
        &mut self,
        layer: IdentityLayer,
        record: &DecodedCanonicalCAbiSignatureFingerprintRecord,
    ) -> Result<(), IdentityValidationError> {
        self.require_registration_phase()?;
        let fingerprint = self.verify_c_abi_signature(record)?;
        self.insert_candidate(layer, fingerprint)
    }

    pub fn register_c_abi_layout(
        &mut self,
        layer: IdentityLayer,
        record: &DecodedCanonicalCAbiLayoutFingerprintRecord,
    ) -> Result<(), IdentityValidationError> {
        self.require_registration_phase()?;
        let fingerprint = self.verify_c_abi_layout(record)?;
        self.insert_candidate(layer, fingerprint)
    }

    pub fn register_native_external_contract(
        &mut self,
        layer: IdentityLayer,
        record: &DecodedNativeExternalContractRecord,
    ) -> Result<(), IdentityValidationError> {
        self.require_registration_phase()?;
        let fingerprint = self.verify_native_external_contract(record)?;
        let node = IdentityNode::trusted(fingerprint);
        match self.candidates.get(&node) {
            None => self.insert_candidate(layer, fingerprint),
            Some(candidate) if candidate.layer == Some(layer) && !candidate.resolved => Ok(()),
            Some(_) => self.fail(IdentityValidationError::DuplicateIdentity {
                kind: node.kind,
                id: node.bytes,
            }),
        }
    }

    fn insert_resolved_leaf<I: PersistentId + 'static>(
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
        self.reserve_candidate_slot()?;
        self.candidates.insert(
            node,
            Candidate {
                trusted_id: Arc::new(id),
                layer: None,
                resolved: true,
                dependency_count: 0,
                dependents: Vec::new(),
            },
        );
        Ok(())
    }

    fn register_external_reference<I>(
        &mut self,
        id: DecodedPersistentId<I>,
    ) -> Result<(), IdentityValidationError>
    where
        I: PersistentId + PersistentIdConstruction + 'static,
    {
        self.require_registration_phase()?;
        self.insert_resolved_leaf(I::from_digest(Digest256::from_array(*id.as_array())))
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

        self.insert_candidate(layer, expected)
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
        self.insert_candidate(layer, expected)
    }

    pub fn register_callable_body(
        &mut self,
        layer: IdentityLayer,
        record: &DecodedRuntimeIdentityRecord<PersistentCallableBodyId>,
    ) -> Result<(), IdentityValidationError> {
        self.require_registration_phase()?;

        let decoded = record.decoded_id();
        let validated = match record.validate_key::<DecodedCallableBodyKey>() {
            Ok(validated) => validated,
            Err(error) => {
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
        };
        self.insert_candidate(layer, validated.0.id())
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

        let record = record.clone();

        let resolved = {
            let mut resolver = PendingIdentityResolver {
                current: node,
                candidates: &mut self.candidates,
                canonical_keys: &self.canonical_keys,

                resource_error: &mut self.resource_error,
                resource_path: &self.resource_path,
            };
            record.resolve(|key| {
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
        self.store_resolved_key::<I, D::Canonical>(node, resolved.into_key())
    }

    pub fn resolve_source_native_contract(
        &mut self,
        record: &DecodedSourceNativeExternalContractRecord,
    ) -> Result<(), IdentityValidationError> {
        let node = IdentityNode::decoded(record.decoded_id());
        self.start_resolution(node)?;

        let record = record.clone();
        let resolved = {
            let mut resolver = PendingIdentityResolver {
                current: node,
                candidates: &mut self.candidates,
                canonical_keys: &self.canonical_keys,

                resource_error: &mut self.resource_error,
                resource_path: &self.resource_path,
            };
            record.resolve(&mut resolver)
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

    pub fn resolve_c_abi_layout(
        &mut self,
        record: &DecodedCanonicalCAbiLayoutFingerprintRecord,
    ) -> Result<(), IdentityValidationError> {
        let fingerprint = self.verify_c_abi_layout(record)?;
        let record = record.clone();
        self.resolve_verified_leaf(fingerprint, |resolver| {
            record.resolve(resolver).map(|record| record.into_layout())
        })
    }

    pub fn resolve_c_abi_signature(
        &mut self,
        record: &DecodedCanonicalCAbiSignatureFingerprintRecord,
    ) -> Result<(), IdentityValidationError> {
        let fingerprint = self.verify_c_abi_signature(record)?;
        let record = record.clone();
        self.resolve_verified_leaf(fingerprint, |resolver| {
            record
                .resolve(resolver)
                .map(|record| record.into_signature())
        })
    }

    pub fn resolve_native_external_contract(
        &mut self,
        record: &DecodedNativeExternalContractRecord,
    ) -> Result<(), IdentityValidationError> {
        let fingerprint = self.verify_native_external_contract(record)?;
        let node = IdentityNode::trusted(fingerprint);
        let first_preimage = self.start_shared_resolution(node)?;

        let record = record.clone();
        let resolved = {
            let mut resolver = PendingIdentityResolver {
                current: node,
                candidates: &mut self.candidates,
                canonical_keys: &self.canonical_keys,

                resource_error: &mut self.resource_error,
                resource_path: &self.resource_path,
            };
            record.resolve_fingerprint_input(&mut resolver)
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
        if first_preimage {
            self.store_resolved_key::<
                NativeExternalContractFingerprint,
                NativeExternalContractFingerprintInput,
            >(node, resolved)
        } else {
            self.require_matching_resolved_key::<
                NativeExternalContractFingerprint,
                NativeExternalContractFingerprintInput,
            >(node, &resolved)
        }
    }

    fn resolve_verified_leaf<I, K, E>(
        &mut self,
        id: I,
        resolve: impl FnOnce(&mut PendingIdentityResolver<'_>) -> Result<Arc<K>, E>,
    ) -> Result<(), IdentityValidationError>
    where
        I: PersistentId + 'static,
        K: Eq + Send + Sync + 'static,
        E: fmt::Display,
    {
        let node = IdentityNode::trusted(id);
        self.start_resolution(node)?;

        let resolved = {
            let mut resolver = PendingIdentityResolver {
                current: node,
                candidates: &mut self.candidates,
                canonical_keys: &self.canonical_keys,

                resource_error: &mut self.resource_error,
                resource_path: &self.resource_path,
            };
            resolve(&mut resolver)
        };
        self.require_no_resource_error()?;
        match resolved {
            Ok(key) => self.store_resolved_key_arc::<I, K>(node, key),
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
        if let Some(node) = self
            .candidates
            .iter()
            .filter_map(|(node, candidate)| {
                (candidate.layer.is_some() && !candidate.resolved).then_some(*node)
            })
            .min()
        {
            return Err(IdentityValidationError::UnresolvedIdentity {
                kind: node.kind,
                id: node.bytes,
            });
        }
        let node_count = u64::try_from(self.candidates.len())
            .map_err(|_| resource_error(WireErrorKind::IntegerOutOfRange, &self.resource_path))?;
        if let Some(node) = find_cycle(&mut self.candidates, node_count, &self.resource_path)? {
            return Err(IdentityValidationError::DependencyCycle {
                kind: node.kind,
                id: node.bytes,
            });
        }
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

    fn verify_c_abi_signature(
        &mut self,
        record: &DecodedCanonicalCAbiSignatureFingerprintRecord,
    ) -> Result<CanonicalCAbiSignatureFingerprint, IdentityValidationError> {
        let fingerprint = match record.candidate_fingerprint() {
            Ok(fingerprint) => fingerprint,
            Err(error) => {
                return self.fail(IdentityValidationError::Hash {
                    kind: CanonicalCAbiSignatureFingerprint::KIND,
                    error,
                });
            }
        };
        self.verify_decoded_leaf(record.decoded_fingerprint(), fingerprint)?;
        Ok(fingerprint)
    }

    fn verify_c_abi_layout(
        &mut self,
        record: &DecodedCanonicalCAbiLayoutFingerprintRecord,
    ) -> Result<CanonicalCAbiLayoutFingerprint, IdentityValidationError> {
        let fingerprint = match record.candidate_fingerprint() {
            Ok(fingerprint) => fingerprint,
            Err(error) => {
                return self.fail(IdentityValidationError::Hash {
                    kind: CanonicalCAbiLayoutFingerprint::KIND,
                    error,
                });
            }
        };
        self.verify_decoded_leaf(record.decoded_fingerprint(), fingerprint)?;
        Ok(fingerprint)
    }

    fn verify_native_external_contract(
        &mut self,
        record: &DecodedNativeExternalContractRecord,
    ) -> Result<NativeExternalContractFingerprint, IdentityValidationError> {
        let copy = record.clone();
        let plan = match copy.into_fingerprint_hash_plan() {
            Ok(plan) => plan,
            Err(error) => return self.fail_native_fingerprint(record, error),
        };

        let fingerprint = match plan.candidate_fingerprint() {
            Ok(fingerprint) => fingerprint,
            Err(error) => return self.fail_native_fingerprint(record, error),
        };
        self.verify_decoded_leaf(record.decoded_fingerprint(), fingerprint)?;
        Ok(fingerprint)
    }

    fn fail_native_fingerprint<T>(
        &mut self,
        record: &DecodedNativeExternalContractRecord,
        error: NativeExternalContractFingerprintError,
    ) -> Result<T, IdentityValidationError> {
        match error {
            NativeExternalContractFingerprintError::Hash(error) => {
                self.fail(IdentityValidationError::Hash {
                    kind: NativeExternalContractFingerprint::KIND,
                    error,
                })
            }
            error => self.fail(IdentityValidationError::InvalidRecord {
                kind: NativeExternalContractFingerprint::KIND,
                id: *record.decoded_fingerprint().as_array(),
                reason: error.to_string(),
            }),
        }
    }

    fn verify_decoded_leaf<I: PersistentId>(
        &mut self,
        decoded: DecodedPersistentId<I>,
        expected: I,
    ) -> Result<(), IdentityValidationError> {
        match decoded.verify(expected) {
            Ok(_) => Ok(()),
            Err(mismatch) => self.fail(IdentityValidationError::IdentityMismatch {
                kind: I::KIND,
                expected: *mismatch.expected().as_array(),
                actual: *mismatch.actual(),
            }),
        }
    }

    fn insert_candidate<I: PersistentId + 'static>(
        &mut self,
        layer: IdentityLayer,
        id: I,
    ) -> Result<(), IdentityValidationError> {
        let node = IdentityNode::trusted(id);
        if self.candidates.contains_key(&node) {
            return self.fail(IdentityValidationError::DuplicateIdentity {
                kind: node.kind,
                id: node.bytes,
            });
        }
        self.reserve_candidate_slot()?;
        self.candidates.insert(
            node,
            Candidate {
                trusted_id: Arc::new(id),
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

    fn start_shared_resolution(
        &mut self,
        node: IdentityNode,
    ) -> Result<bool, IdentityValidationError> {
        if self.phase == ValidationPhase::Poisoned {
            return Err(IdentityValidationError::Poisoned);
        }
        self.phase = ValidationPhase::Resolving;
        match self.candidates.get(&node) {
            Some(candidate) if candidate.layer.is_some() => Ok(!candidate.resolved),
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
        K: Eq + Send + Sync + 'static,
    {
        self.store_resolved_key_arc::<I, K>(node, Arc::new(key))
    }

    fn store_resolved_key_arc<I, K>(
        &mut self,
        node: IdentityNode,
        key: Arc<K>,
    ) -> Result<(), IdentityValidationError>
    where
        I: PersistentId + 'static,
        K: Eq + Send + Sync + 'static,
    {
        let slot = CanonicalKeySlot::new::<I, K>(node.bytes);
        if self.canonical_keys.contains_key(&slot) {
            return self.fail(IdentityValidationError::AlreadyResolved {
                kind: node.kind,
                id: node.bytes,
            });
        }
        if !self.candidates.contains_key(&node) {
            return self.fail(IdentityValidationError::UnregisteredIdentity {
                kind: node.kind,
                id: node.bytes,
            });
        }
        self.reserve_canonical_key_slot()?;
        self.canonical_keys.insert(slot, key);
        let Some(candidate) = self.candidates.get_mut(&node) else {
            return self.fail(IdentityValidationError::UnregisteredIdentity {
                kind: node.kind,
                id: node.bytes,
            });
        };
        candidate.resolved = true;
        Ok(())
    }

    fn reserve_candidate_slot(&mut self) -> Result<(), IdentityValidationError> {
        let result =
            scoop_wire::allocation::try_reserve_map(&mut self.candidates, 1, &self.resource_path);
        match result {
            Ok(()) => Ok(()),
            Err(error) => self.fail(IdentityValidationError::Resource(error)),
        }
    }

    fn reserve_canonical_key_slot(&mut self) -> Result<(), IdentityValidationError> {
        let result = scoop_wire::allocation::try_reserve_map(
            &mut self.canonical_keys,
            1,
            &self.resource_path,
        );
        match result {
            Ok(()) => Ok(()),
            Err(error) => self.fail(IdentityValidationError::Resource(error)),
        }
    }

    fn require_matching_resolved_key<I, K>(
        &mut self,
        node: IdentityNode,
        key: &K,
    ) -> Result<(), IdentityValidationError>
    where
        I: PersistentId + 'static,
        K: Eq + Send + Sync + 'static,
    {
        let slot = CanonicalKeySlot::new::<I, K>(node.bytes);
        match self.canonical_keys.get(&slot) {
            Some(existing) if existing.equals(key) => Ok(()),
            Some(_) => self.fail(IdentityValidationError::IdentityCollision {
                kind: node.kind,
                id: node.bytes,
            }),
            None => self.fail(IdentityValidationError::InvalidRecord {
                kind: node.kind,
                id: node.bytes,
                reason: "resolved identity is missing its canonical key".to_owned(),
            }),
        }
    }

    fn fail<T>(&mut self, error: IdentityValidationError) -> Result<T, IdentityValidationError> {
        self.phase = ValidationPhase::Poisoned;
        Err(error)
    }

    fn poison(&mut self, error: IdentityValidationError) -> IdentityValidationError {
        self.phase = ValidationPhase::Poisoned;
        error
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
    candidates: &'validation mut HashMap<IdentityNode, Candidate>,
    canonical_keys: &'validation HashMap<CanonicalKeySlot, Arc<dyn ErasedCanonicalKey>>,

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
            .and_then(|candidate| candidate.layer)
            .ok_or(IdentityReferenceError::Missing {
                kind: self.current.kind,
                id: self.current.bytes,
            })?;
        if let Some(target_layer) = target_layer
            && target_layer > source_layer
        {
            return Err(IdentityReferenceError::FutureLayer {
                source: source_layer,
                target: target_layer,
                kind: target.kind,
                id: target.bytes,
            });
        }
        let Some(source_candidate) = self.candidates.get(&self.current) else {
            return Err(IdentityReferenceError::Missing {
                kind: self.current.kind,
                id: self.current.bytes,
            });
        };
        let next_dependency_count = match source_candidate.dependency_count.checked_add(1) {
            Some(count) => count,
            None => {
                *self.resource_error = Some(WireError::new(
                    WireErrorKind::IntegerOutOfRange,
                    self.resource_path.clone(),
                    None,
                ));
                return Err(IdentityReferenceError::StorageFailure);
            }
        };

        let Some(target_candidate) = self.candidates.get_mut(&target) else {
            return Err(IdentityReferenceError::Missing {
                kind: target.kind,
                id: target.bytes,
            });
        };
        if target_candidate.dependents.try_reserve_exact(1).is_err() {
            *self.resource_error = Some(WireError::new(
                WireErrorKind::Allocation,
                self.resource_path.clone(),
                None,
            ));
            return Err(IdentityReferenceError::StorageFailure);
        }
        target_candidate.dependents.push(self.current);
        let Some(source_candidate) = self.candidates.get_mut(&self.current) else {
            return Err(IdentityReferenceError::Missing {
                kind: self.current.kind,
                id: self.current.bytes,
            });
        };
        source_candidate.dependency_count = next_dependency_count;
        Ok(I::from_digest(Digest256::from_array(target.bytes)))
    }
}

impl<I, K> PersistentKeyResolver<I, K> for PendingIdentityResolver<'_>
where
    I: PersistentId + PersistentIdConstruction + 'static,
    K: Send + Sync + 'static,
{
    type Error = IdentityReferenceError;

    fn resolve_key(&mut self, id: DecodedPersistentId<I>) -> Result<Arc<K>, Self::Error> {
        let resolved = <Self as PersistentIdResolver<I>>::resolve(self, id)?;
        let slot = CanonicalKeySlot::new::<I, K>(*resolved.as_array());
        self.canonical_keys
            .get(&slot)
            .cloned()
            .and_then(|key| key.into_any().downcast::<K>().ok())
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
    candidates: HashMap<IdentityNode, Candidate>,
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

    /// Returns the canonical key already proven for one trusted typed id.
    ///
    /// This is the read-only counterpart of [`PersistentKeyResolver`]: the
    /// caller already holds an `I`, so no untrusted bytes are promoted here.
    /// The lookup remains kind- and key-type-specific and never falls back to
    /// another identity family that happens to share the same digest bytes.
    pub fn canonical_key<I, K>(&self, id: I) -> Result<Arc<K>, IdentityReferenceError>
    where
        I: PersistentId + 'static,
        K: Send + Sync + 'static,
    {
        let node = IdentityNode::trusted(id);
        self.candidates
            .get(&node)
            .filter(|candidate| candidate.resolved)
            .ok_or(IdentityReferenceError::Missing {
                kind: node.kind,
                id: node.bytes,
            })?;
        let slot = CanonicalKeySlot::new::<I, K>(node.bytes);
        self.canonical_keys
            .get(&slot)
            .cloned()
            .and_then(|key| key.into_any().downcast::<K>().ok())
            .ok_or(IdentityReferenceError::KeyUnavailable {
                kind: node.kind,
                id: node.bytes,
            })
    }

    pub(crate) fn canonical_key_ref<I, K>(&self, id: I) -> Option<&K>
    where
        I: PersistentId + 'static,
        K: Send + Sync + 'static,
    {
        let node = IdentityNode::trusted(id);
        self.candidates
            .get(&node)
            .filter(|candidate| candidate.resolved)?;
        self.canonical_keys
            .get(&CanonicalKeySlot::new::<I, K>(node.bytes))?
            .as_any()
            .downcast_ref::<K>()
    }

    /// Tests whether an already typed identity is present in this graph.
    pub fn contains_resolved_identity<I: PersistentId>(&self, id: I) -> bool {
        self.candidates
            .get(&IdentityNode::trusted(id))
            .is_some_and(|candidate| candidate.resolved)
    }

    /// Returns the already checked record while sharing its canonical key.
    /// This does not introduce another identity or resolve an unchecked id.
    pub fn canonical_record<I, K>(
        &self,
        id: I,
    ) -> Result<CborIdentityRecord<I, K>, IdentityReferenceError>
    where
        I: PersistentId + 'static,
        K: CborIdentityKey<I> + Send + Sync + 'static,
    {
        self.canonical_key(id)
            .map(|key| CborIdentityRecord::from_verified_shared(id, key))
    }

    /// Reconstructs runtime-encoded records introduced by one layer and key
    /// family, sorted by raw persistent id.
    pub fn runtime_records<I, K>(
        &self,
        layer: IdentityLayer,

        path: &WirePath,
    ) -> Result<Vec<RuntimeIdentityRecord<I, K>>, IdentityValidationError>
    where
        I: PersistentId + 'static,
        K: RuntimeIdentityKey<I> + Clone + Send + Sync + 'static,
    {
        let record_count = self
            .candidates
            .iter()
            .filter(|(node, candidate)| {
                node.kind == I::KIND
                    && candidate.layer == Some(layer)
                    && self
                        .canonical_keys
                        .contains_key(&CanonicalKeySlot::new::<I, K>(node.bytes))
            })
            .count();
        let record_count = u64::try_from(record_count).map_err(|_| {
            IdentityValidationError::Resource(WireError::new(
                WireErrorKind::IntegerOutOfRange,
                path.clone(),
                None,
            ))
        })?;
        let mut records = Vec::new();
        scoop_wire::allocation::try_reserve_count(&mut records, record_count, path)
            .map_err(IdentityValidationError::Resource)?;
        for (node, candidate) in &self.candidates {
            if node.kind != I::KIND || candidate.layer != Some(layer) {
                continue;
            }
            let slot = CanonicalKeySlot::new::<I, K>(node.bytes);
            let Some(key) = self
                .canonical_keys
                .get(&slot)
                .cloned()
                .and_then(|key| key.into_any().downcast::<K>().ok())
            else {
                continue;
            };
            let Some(id) = candidate.trusted_id.downcast_ref::<I>().copied() else {
                return Err(IdentityValidationError::InvalidRecord {
                    kind: I::KIND,
                    id: node.bytes,
                    reason: "validated identity has the wrong concrete id type".to_owned(),
                });
            };
            let record = RuntimeIdentityRecord::from_verified_key(id, key).map_err(|error| {
                IdentityValidationError::InvalidRecord {
                    kind: I::KIND,
                    id: node.bytes,
                    reason: error.to_string(),
                }
            })?;
            records.push(record);
        }
        records.sort_by_key(RuntimeIdentityRecord::id);
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
    K: Send + Sync + 'static,
{
    type Error = IdentityReferenceError;

    fn resolve_key(&mut self, id: DecodedPersistentId<I>) -> Result<Arc<K>, Self::Error> {
        let resolved = <Self as PersistentIdResolver<I>>::resolve(self, id)?;
        let slot = CanonicalKeySlot::new::<I, K>(*resolved.as_array());
        self.canonical_keys
            .get(&slot)
            .cloned()
            .and_then(|key| key.into_any().downcast::<K>().ok())
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
    StorageFailure,
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
            Self::StorageFailure => {
                formatter.write_str("identity storage allocation or size failed")
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
    IdentityCollision {
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
            Self::IdentityCollision { kind, id } => {
                write!(
                    formatter,
                    "conflicting canonical keys share {kind} identity "
                )?;
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
    candidates: &mut HashMap<IdentityNode, Candidate>,
    node_count: u64,
    path: &WirePath,
) -> Result<Option<IdentityNode>, IdentityValidationError> {
    let capacity = usize::try_from(node_count)
        .map_err(|_| resource_error(WireErrorKind::IntegerOutOfRange, path))?;
    let mut ready_storage = Vec::new();
    ready_storage.try_reserve_exact(capacity).map_err(|_| {
        IdentityValidationError::Resource(WireError::new(
            WireErrorKind::Allocation,
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
        completed = completed
            .checked_add(1)
            .ok_or_else(|| resource_error(WireErrorKind::IntegerOutOfRange, path))?;
        let Some(candidate) = candidates.get_mut(&node) else {
            return Err(IdentityValidationError::UnregisteredIdentity {
                kind: node.kind,
                id: node.bytes,
            });
        };
        let dependents = std::mem::take(&mut candidate.dependents);
        for dependent in dependents {
            let Some(candidate) = candidates.get_mut(&dependent) else {
                return Err(IdentityValidationError::UnregisteredIdentity {
                    kind: dependent.kind,
                    id: dependent.bytes,
                });
            };
            candidate.dependency_count = candidate
                .dependency_count
                .checked_sub(1)
                .ok_or_else(|| resource_error(WireErrorKind::IntegerOutOfRange, path))?;
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
            .filter_map(|(node, candidate)| (candidate.dependency_count != 0).then_some(*node))
            .min())
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
