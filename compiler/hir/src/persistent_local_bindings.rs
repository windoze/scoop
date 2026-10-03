//! Persistent identities for current-Cone name-resolution bindings.

use std::fmt;

use scoop_identity::{
    CborIdentityRecord, DefinitionOrigin, LocalBindingKey, PersistentLocalBindingId,
};
use scoop_wire::HashError;

pub type HirLocalBindingRecord = CborIdentityRecord<PersistentLocalBindingId, LocalBindingKey>;

/// One canonical current-Cone binding and the deterministic source origin
/// used by the identity-foundation projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirLocalBindingIdentity {
    record: HirLocalBindingRecord,
    origin: DefinitionOrigin,
}

impl HirLocalBindingIdentity {
    pub fn new(
        key: LocalBindingKey,
        origin: DefinitionOrigin,
    ) -> Result<Self, HirLocalBindingIdentityError> {
        if key.source() != origin.source() {
            return Err(HirLocalBindingIdentityError::OriginSourceMismatch);
        }
        let record =
            CborIdentityRecord::from_key(key).map_err(HirLocalBindingIdentityError::Identity)?;
        Ok(Self { record, origin })
    }

    pub const fn record(&self) -> &HirLocalBindingRecord {
        &self.record
    }

    pub const fn origin(&self) -> &DefinitionOrigin {
        &self.origin
    }
}

/// Canonically ordered current-Cone binding identities.
///
/// Repeated semantically identical imports in one source intentionally share
/// an identity. Their foundation origin is the least stable source origin,
/// independent of source traversal or insertion order.
#[derive(Clone, Debug, Default)]
pub struct HirLocalBindingIdentities {
    identities: Vec<HirLocalBindingIdentity>,
}

impl HirLocalBindingIdentities {
    pub fn canonicalize(
        mut identities: Vec<HirLocalBindingIdentity>,
    ) -> Result<Self, HirLocalBindingIdentityError> {
        identities.sort_by(|left, right| {
            left.record
                .id()
                .cmp(&right.record.id())
                .then_with(|| left.origin.cmp(&right.origin))
        });

        let mut canonical = Vec::<HirLocalBindingIdentity>::new();
        canonical
            .try_reserve_exact(identities.len())
            .map_err(|_| HirLocalBindingIdentityError::Allocation)?;
        for identity in identities {
            if let Some(previous) = canonical.last() {
                if previous.record.id() == identity.record.id() {
                    if previous.record != identity.record {
                        return Err(HirLocalBindingIdentityError::Collision(
                            identity.record.id(),
                        ));
                    }
                    continue;
                }
            }
            canonical.push(identity);
        }
        Ok(Self {
            identities: canonical,
        })
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirLocalBindingIdentity> {
        self.identities.iter()
    }

    pub fn len(&self) -> usize {
        self.identities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.identities.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirLocalBindingIdentityError {
    OriginSourceMismatch,
    Identity(HashError),
    Collision(PersistentLocalBindingId),
    Allocation,
}

impl fmt::Display for HirLocalBindingIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OriginSourceMismatch => {
                formatter.write_str("local binding origin must use the binding source")
            }
            Self::Identity(error) => error.fmt(formatter),
            Self::Collision(identity) => {
                write!(
                    formatter,
                    "distinct local binding keys collide at {identity}"
                )
            }
            Self::Allocation => {
                formatter.write_str("failed to allocate canonical local binding identities")
            }
        }
    }
}

impl std::error::Error for HirLocalBindingIdentityError {}

#[cfg(test)]
mod tests;
