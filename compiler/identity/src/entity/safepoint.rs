use std::fmt;
use std::num::NonZeroU64;

use scoop_wire::{
    Encoder, HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, WireEncode,
    domain_separated_raw_hash,
};

use crate::ids::derive_persistent_id;
use crate::{PersistentCallableBodyId, PersistentExactTypeId, PersistentSafepointSiteId};

mod decode;

pub use decode::DecodedSafepointSiteKey;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SafepointSiteRole {
    ManagedPoll,
    ManagedCall,
    ManagedInvoke,
    NativeSafeTransition,
    NativeBorrowedTransition,
}

impl SafepointSiteRole {
    pub const fn tag(self) -> u32 {
        match self {
            Self::ManagedPoll => 1,
            Self::ManagedCall => 2,
            Self::ManagedInvoke => 3,
            Self::NativeSafeTransition => 4,
            Self::NativeBorrowedTransition => 5,
        }
    }
}

impl WireEncode for SafepointSiteRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.tag()))
    }
}

impl RuntimeEncode for SafepointSiteRole {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.u32(self.tag())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SafepointSiteKey {
    owner: PersistentCallableBodyId,
    role: SafepointSiteRole,
    ordinal: u32,
}

impl SafepointSiteKey {
    pub const fn new(
        owner: PersistentCallableBodyId,
        role: SafepointSiteRole,
        ordinal: u32,
    ) -> Self {
        Self {
            owner,
            role,
            ordinal,
        }
    }

    pub const fn owner(self) -> PersistentCallableBodyId {
        self.owner
    }

    pub const fn role(self) -> SafepointSiteRole {
        self.role
    }

    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }
}

impl WireEncode for SafepointSiteKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)?;
        encoder.field(3)?;
        encoder.unsigned(u64::from(self.ordinal))
    }
}

impl PersistentSafepointSiteId {
    pub fn from_key(key: &SafepointSiteKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-safepoint-site-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeTypeId(NonZeroU64);

impl RuntimeTypeId {
    const HASH_DOMAIN: &'static str = "scoop-runtime-type-id-v1";

    pub fn derive(exact_type: PersistentExactTypeId) -> Result<Self, DerivedIdError> {
        derive_nonzero_u64(Self::HASH_DOMAIN, exact_type.as_array()).map(Self)
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SafepointId(NonZeroU64);

impl SafepointId {
    const HASH_DOMAIN: &'static str = "scoop-safepoint-id-v1";

    pub fn derive(site: PersistentSafepointSiteId) -> Result<Self, DerivedIdError> {
        derive_nonzero_u64(Self::HASH_DOMAIN, site.as_array()).map(Self)
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

macro_rules! impl_derived_id_encoding {
    ($name:ident) => {
        impl WireEncode for $name {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.unsigned(self.0.get())
            }
        }

        impl RuntimeEncode for $name {
            fn runtime_encode(
                &self,
                encoder: &mut RuntimeEncoder,
            ) -> Result<(), RuntimeEncodeError> {
                encoder.u64(self.0.get())
            }
        }
    };
}

impl_derived_id_encoding!(RuntimeTypeId);
impl_derived_id_encoding!(SafepointId);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerivedIdError {
    Zero,
    Hash(HashError),
}

impl fmt::Display for DerivedIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => formatter.write_str("derived runtime id is zero"),
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DerivedIdError {}

fn derive_nonzero_u64(domain: &str, raw: &[u8; 32]) -> Result<NonZeroU64, DerivedIdError> {
    let digest = domain_separated_raw_hash(domain, raw).map_err(DerivedIdError::Hash)?;
    let bytes = digest.as_array();
    let value = u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]);
    NonZeroU64::new(value).ok_or(DerivedIdError::Zero)
}

#[cfg(test)]
mod tests {
    use scoop_wire::{encode, encode_runtime};

    use super::{RuntimeTypeId, SafepointId, SafepointSiteKey, SafepointSiteRole};
    use crate::{
        ConeIdentity, PersistentCallableBodyId, PersistentExactTypeId, PersistentSafepointSiteId,
    };

    #[test]
    fn safepoint_site_identity_keeps_role_local_ordinal() {
        let owner = PersistentCallableBodyId(ConeIdentity::CORE.0);
        let key = SafepointSiteKey::new(owner, SafepointSiteRole::ManagedInvoke, 7);
        let site = PersistentSafepointSiteId::from_key(&key).unwrap();

        assert_eq!(
            hex(&encode(&key).unwrap()),
            format!("a3015820{owner}02030307")
        );
        assert_eq!(
            site.to_string(),
            "7043dd5d994a47bd9db860af5dc720a6b0bc25eb121cee6cf03236dd4caf1e69"
        );
    }

    #[test]
    fn runtime_type_and_safepoint_ids_use_distinct_nonzero_derivations() {
        let raw = ConeIdentity::CORE.0;
        let runtime_type = RuntimeTypeId::derive(PersistentExactTypeId(raw)).unwrap();
        let safepoint = SafepointId::derive(PersistentSafepointSiteId(raw)).unwrap();

        assert_eq!(runtime_type.get(), 3_993_245_273_086_009_875);
        assert_eq!(safepoint.get(), 14_572_348_639_280_067_605);
    }

    #[test]
    fn safepoint_roles_share_the_same_frozen_wire_and_runtime_tags() {
        let roles = [
            SafepointSiteRole::ManagedPoll,
            SafepointSiteRole::ManagedCall,
            SafepointSiteRole::ManagedInvoke,
            SafepointSiteRole::NativeSafeTransition,
            SafepointSiteRole::NativeBorrowedTransition,
        ];

        for (role, tag) in roles.into_iter().zip(1_u8..=5) {
            assert_eq!(encode(&role).unwrap(), [tag]);
            assert_eq!(encode_runtime(&role).unwrap(), [tag, 0, 0, 0]);
        }
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
