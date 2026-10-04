//! Identities for task-context keys and compiler-owned storage.

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{ConeIdentity, PersistentExactTypeId};

/// An exact source type used as a context key. It has no additional digest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContextKey(pub PersistentExactTypeId);

impl WireEncode for ContextKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

/// Only Task and Node have heap instances; the other roles describe values.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ContextStorageRole {
    Task,
    Node,
    Binding,
    Mark,
    SwitchGuard,
}

impl ContextStorageRole {
    pub const ALL: [Self; 5] = [
        Self::Task,
        Self::Node,
        Self::Binding,
        Self::Mark,
        Self::SwitchGuard,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Task => "task-context",
            Self::Node => "context-node",
            Self::Binding => "context-binding-ref",
            Self::Mark => "context-mark",
            Self::SwitchGuard => "context-switch-guard",
        }
    }

    pub const fn is_reference(self) -> bool {
        matches!(self, Self::Task | Self::Node | Self::Binding)
    }
}

impl WireEncode for ContextStorageRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Task => 1,
            Self::Node => 2,
            Self::Binding => 3,
            Self::Mark => 4,
            Self::SwitchGuard => 5,
        })
    }
}

impl WireDecode for ContextStorageRole {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        Ok(match decoder.unsigned()? {
            1 => Self::Task,
            2 => Self::Node,
            3 => Self::Binding,
            4 => Self::Mark,
            5 => Self::SwitchGuard,
            tag => {
                return Err(WireError::new(
                    WireErrorKind::UnknownTag { tag },
                    decoder.path().clone(),
                    Some(decoder.position()),
                ));
            }
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContextStorageType {
    pub core: ConeIdentity,
    pub role: ContextStorageRole,
}

impl ContextStorageType {
    pub const fn new(core: ConeIdentity, role: ContextStorageRole) -> Self {
        Self { core, role }
    }

    pub fn nominal_record(
        self,
    ) -> crate::CborIdentityRecord<crate::PersistentTypeId, crate::GeneratedNominalKey> {
        crate::CborIdentityRecord::from_key(crate::GeneratedNominalKey::TaskContext(self))
            .expect("a compiler context role has a canonical identity")
    }

    pub fn exact_record(
        self,
    ) -> crate::CborIdentityRecord<PersistentExactTypeId, crate::ExactTypeKey> {
        crate::CborIdentityRecord::from_key(crate::ExactTypeKey::Nominal(
            self.nominal_record().id(),
        ))
        .expect("a compiler context role has an exact nominal identity")
    }
}

impl WireEncode for ContextStorageType {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(2)?;
        self.core.encode(encoder)?;
        self.role.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedContextStorageType {
    pub core: crate::DecodedPersistentId<ConeIdentity>,
    pub role: ContextStorageRole,
}

impl DecodedContextStorageType {
    pub fn resolve<R: crate::PersistentIdResolver<ConeIdentity>>(
        self,
        resolver: &mut R,
    ) -> Result<ContextStorageType, R::Error> {
        Ok(ContextStorageType {
            core: resolver.resolve(self.core)?,
            role: self.role,
        })
    }
}

impl WireEncode for DecodedContextStorageType {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(2)?;
        self.core.encode(encoder)?;
        self.role.encode(encoder)
    }
}

impl WireDecode for DecodedContextStorageType {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let actual = decoder.array()?;
        if actual != 2 {
            return Err(WireError::new(
                WireErrorKind::InvalidLength {
                    expected: 2,
                    actual,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        Ok(Self {
            core: crate::DecodedPersistentId::decode(decoder)?,
            role: ContextStorageRole::decode(decoder)?,
        })
    }
}
