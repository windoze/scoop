use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::any::TypeId;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use super::{CanonicalKeySlot, ErasedCanonicalKey, IdentityLayer, ValidatedIdentityGraph};
use crate::{ConeIdentity, PersistentId};

/// The three semantic fingerprints that identify one Cone's foundation
/// contribution to a compiler session.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SemanticOriginFingerprint {
    hir: [u8; 32],
    mir: [u8; 32],
    lir: [u8; 32],
}

impl SemanticOriginFingerprint {
    pub const fn new(hir: [u8; 32], mir: [u8; 32], lir: [u8; 32]) -> Self {
        Self { hir, mir, lir }
    }

    pub const fn hir(self) -> [u8; 32] {
        self.hir
    }

    pub const fn mir(self) -> [u8; 32] {
        self.mir
    }

    pub const fn lir(self) -> [u8; 32] {
        self.lir
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct SemanticIdentitySlot {
    id_type: TypeId,
    bytes: [u8; 32],
}

impl SemanticIdentitySlot {
    fn from_key_slot(slot: CanonicalKeySlot) -> Self {
        Self {
            id_type: slot.id_type,
            bytes: slot.bytes,
        }
    }

    fn from_id<I: PersistentId + 'static>(id: I) -> Self {
        Self {
            id_type: TypeId::of::<I>(),
            bytes: *id.as_array(),
        }
    }
}

#[derive(Clone)]
struct SessionEntity {
    world_index: u32,
    key_slot: CanonicalKeySlot,
    key: Arc<dyn ErasedCanonicalKey>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SemanticInputOrigin {
    Artifact(SemanticOriginFingerprint),
    Source,
}

#[derive(Clone)]
struct ImportedOrigin {
    input: SemanticInputOrigin,
    graph: ValidatedIdentityGraph,
    layers: ImportedIdentityLayers,
}

/// Session-owned interner for identities imported from validated artifacts.
///
/// An import is checked and allocated completely before either the origin
/// registry or entity table is mutated. A failed import therefore leaves the
/// session byte-for-byte equivalent at the API level.
#[derive(Default)]
pub struct SemanticIdentitySession {
    origins: HashMap<ConeIdentity, ImportedOrigin>,
    entities: Arc<HashMap<SemanticIdentitySlot, SessionEntity>>,
    next_world_index: u64,
}

/// One already validated identity graph queued for an atomic semantic-session
/// import. The graph remains borrowed so batching does not clone canonical
/// identity keys before the transaction is known to succeed.
#[derive(Clone, Copy)]
pub struct SemanticIdentityImport<'graph> {
    origin: ConeIdentity,
    fingerprint: SemanticOriginFingerprint,
    graph: &'graph ValidatedIdentityGraph,
}

impl<'graph> SemanticIdentityImport<'graph> {
    pub const fn new(
        origin: ConeIdentity,
        fingerprint: SemanticOriginFingerprint,
        graph: &'graph ValidatedIdentityGraph,
    ) -> Self {
        Self {
            origin,
            fingerprint,
            graph,
        }
    }

    pub const fn origin(self) -> ConeIdentity {
        self.origin
    }

    pub const fn fingerprint(self) -> SemanticOriginFingerprint {
        self.fingerprint
    }

    pub const fn graph(self) -> &'graph ValidatedIdentityGraph {
        self.graph
    }
}

mod session;

impl IdentityLayer {
    const fn index(self) -> usize {
        match self {
            Self::Hir => 0,
            Self::Mir => 1,
            Self::Lir => 2,
        }
    }
}

/// A session-local identity id. IR crates wrap this type so imported HIR, MIR,
/// and LIR ids cannot be interchanged even for the same persistent kind.
pub struct ImportedIdentityId<I: PersistentId> {
    world_index: u32,
    persistent: I,
    marker: PhantomData<fn() -> I>,
}

impl<I: PersistentId> ImportedIdentityId<I> {
    pub const fn into_u32(self) -> u32 {
        self.world_index
    }

    pub const fn persistent(self) -> I {
        self.persistent
    }
}

impl<I: PersistentId> Clone for ImportedIdentityId<I> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<I: PersistentId> Copy for ImportedIdentityId<I> {}

impl<I: PersistentId> fmt::Debug for ImportedIdentityId<I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ImportedIdentityId")
            .field(&self.world_index)
            .finish()
    }
}

impl<I: PersistentId> PartialEq for ImportedIdentityId<I> {
    fn eq(&self, other: &Self) -> bool {
        self.world_index == other.world_index
    }
}

impl<I: PersistentId> Eq for ImportedIdentityId<I> {}

impl<I: PersistentId> std::hash::Hash for ImportedIdentityId<I> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.world_index.hash(state);
    }
}

/// The identities introduced by one artifact layer after session commit.
#[doc(hidden)]
pub struct ImportedIdentityMap<L: ImportedIdentityLayer> {
    origin: ConeIdentity,
    by_persistent: Arc<HashMap<SemanticIdentitySlot, u32>>,
    marker: PhantomData<fn() -> L>,
}

impl<L: ImportedIdentityLayer> Clone for ImportedIdentityMap<L> {
    fn clone(&self) -> Self {
        Self {
            origin: self.origin,
            by_persistent: Arc::clone(&self.by_persistent),
            marker: PhantomData,
        }
    }
}

impl<L: ImportedIdentityLayer> ImportedIdentityMap<L> {
    fn new(origin: ConeIdentity) -> Self {
        Self {
            origin,
            by_persistent: Arc::default(),
            marker: PhantomData,
        }
    }

    fn reserve(&mut self, count: usize) -> Result<(), SemanticIdentityImportError> {
        Arc::make_mut(&mut self.by_persistent)
            .try_reserve(count)
            .map_err(|_| SemanticIdentityImportError::Allocation {
                requested_slots: count,
            })
    }

    fn insert(&mut self, slot: SemanticIdentitySlot, world_index: u32) {
        Arc::make_mut(&mut self.by_persistent).insert(slot, world_index);
    }

    pub const fn origin(&self) -> ConeIdentity {
        self.origin
    }

    pub const fn layer(&self) -> IdentityLayer {
        L::LAYER
    }

    pub fn len(&self) -> usize {
        self.by_persistent.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_persistent.is_empty()
    }

    pub fn get<I: PersistentId + 'static>(&self, id: I) -> Option<ImportedIdentityId<I>> {
        self.by_persistent
            .get(&SemanticIdentitySlot::from_id(id))
            .copied()
            .map(|world_index| ImportedIdentityId {
                world_index,
                persistent: id,
                marker: PhantomData,
            })
    }
}

mod private {
    pub trait Sealed {}
}

pub trait ImportedIdentityLayer: private::Sealed {
    const LAYER: IdentityLayer;
}

pub enum HirIdentityLayer {}
pub enum MirIdentityLayer {}
pub enum LirIdentityLayer {}

impl private::Sealed for HirIdentityLayer {}
impl private::Sealed for MirIdentityLayer {}
impl private::Sealed for LirIdentityLayer {}

impl ImportedIdentityLayer for HirIdentityLayer {
    const LAYER: IdentityLayer = IdentityLayer::Hir;
}

impl ImportedIdentityLayer for MirIdentityLayer {
    const LAYER: IdentityLayer = IdentityLayer::Mir;
}

impl ImportedIdentityLayer for LirIdentityLayer {
    const LAYER: IdentityLayer = IdentityLayer::Lir;
}

/// One atomic import split into the three stage-specific identity deltas.
#[derive(Clone)]
pub struct ImportedIdentityLayers {
    hir: ImportedIdentityMap<HirIdentityLayer>,
    mir: ImportedIdentityMap<MirIdentityLayer>,
    lir: ImportedIdentityMap<LirIdentityLayer>,
}

impl ImportedIdentityLayers {
    pub fn into_parts(
        self,
    ) -> (
        ImportedIdentityMap<HirIdentityLayer>,
        ImportedIdentityMap<MirIdentityLayer>,
        ImportedIdentityMap<LirIdentityLayer>,
    ) {
        (self.hir, self.mir, self.lir)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticIdentityImportError {
    OriginConflict { origin: ConeIdentity },
    DuplicateBatchOrigin { origin: ConeIdentity },
    IdentityConflict { kind: &'static str, id: [u8; 32] },
    MissingCanonicalKey,
    WorldIdExhausted,
    Allocation { requested_slots: usize },
}

impl fmt::Display for SemanticIdentityImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OriginConflict { origin } => {
                write!(
                    formatter,
                    "Cone {origin} was already imported from a different input"
                )
            }
            Self::DuplicateBatchOrigin { origin } => {
                write!(
                    formatter,
                    "Cone {origin} occurs more than once in one import batch"
                )
            }
            Self::IdentityConflict { kind, id } => {
                write!(
                    formatter,
                    "{kind} identity {} has a conflicting canonical key",
                    Hex(id)
                )
            }
            Self::MissingCanonicalKey => {
                formatter.write_str("validated identity graph is missing a canonical key")
            }
            Self::WorldIdExhausted => formatter.write_str("semantic world id space is exhausted"),
            Self::Allocation { requested_slots } => write!(
                formatter,
                "failed to allocate semantic import storage for {requested_slots} identities"
            ),
        }
    }
}

impl std::error::Error for SemanticIdentityImportError {}

struct Hex<'a>(&'a [u8; 32]);

impl fmt::Display for Hex<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}
