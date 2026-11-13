use serde::{Deserialize, Serialize};

use crate::{BodyId, Fingerprint};

pub const SCHEMA: u32 = 1;

/// A logical body and the exact pre-optimization body/layout being measured.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyKey {
    pub body: BodyId,
    pub fingerprint: Fingerprint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PositionKind {
    Entry,
    Block,
    Call,
    ThenEdge,
    ElseEdge,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub kind: PositionKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyLayout {
    pub key: BodyKey,
    /// Human-readable only; matching always uses key and position.
    pub name: String,
    /// Position zero is the unique logical entry.
    pub positions: Vec<Position>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub body: BodyKey,
    pub position: u32,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InlineOrigin {
    Call(Location),
    /// A compiler-created call after logical positions were established.
    /// This ordinal distinguishes physical copies within their owning record;
    /// it is never used to match a logical position across builds.
    Generated(u32),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Counter {
    pub location: Location,
    /// Outermost first. Empty for an out-of-line body. This records actual
    /// copies so a call and its entry can be aggregated over the same scope.
    pub inlined_at: Vec<InlineOrigin>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub name: String,
    pub hash: u64,
    pub counters: Vec<Counter>,
}

impl Record {
    /// Assign one slot to each retained logical location and actual inline
    /// scope. Repeated increments of that same location share the slot.
    pub fn new(name: String, mut counters: Vec<Counter>) -> Self {
        counters.sort();
        counters.dedup();
        let mut hash = scoop_wire::CanonicalHashStream::new();
        hash.update_raw(b"scoop-physical-profile-v1");
        hash.update_raw(&(counters.len() as u64).to_le_bytes());
        for counter in &counters {
            hash_location(&mut hash, counter.location);
            hash.update_raw(&(counter.inlined_at.len() as u64).to_le_bytes());
            for origin in &counter.inlined_at {
                match origin {
                    InlineOrigin::Call(site) => {
                        hash.update_raw(&[0]);
                        hash_location(&mut hash, *site);
                    }
                    InlineOrigin::Generated(copy) => {
                        hash.update_raw(&[1]);
                        hash.update_raw(&copy.to_le_bytes());
                    }
                }
            }
        }
        let hash = hash.finalize();
        let hash = u64::from_le_bytes(
            hash.as_array()[..8]
                .try_into()
                .expect("layout hash contains eight bytes"),
        );
        Self {
            name,
            hash,
            counters,
        }
    }
}

fn hash_location(hash: &mut scoop_wire::CanonicalHashStream, location: Location) {
    hash.update_raw(location.body.body.as_array());
    hash.update_raw(location.body.fingerprint.as_array());
    hash.update_raw(&location.position.to_le_bytes());
}

/// The complete logical layouts and the counters that actually survived.
/// A logical position absent from records is unavailable, not observed zero.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileMap {
    pub schema: u32,
    pub target: String,
    pub bodies: Vec<BodyLayout>,
    pub records: Vec<Record>,
}

/// Portable relative references written by the PGO command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileIndex {
    pub schema: u32,
    pub target: String,
    pub counts: String,
    pub maps: Vec<String>,
}

/// LLVM names are bytes. Only a Scoop record map assigns logical identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedRecord {
    pub name: Vec<u8>,
    pub hash: u64,
    pub counts: Vec<u64>,
}
