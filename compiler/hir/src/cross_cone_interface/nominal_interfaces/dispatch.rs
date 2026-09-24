//! Declaration order shared by ordinary metadata and inheritance replay.

use std::collections::BTreeSet;

use scoop_identity::{PersistentDispatchSlotId, SignatureTypeKey};
use scoop_wire::{Encoder, WireEncode};

use crate::{InterfaceSourceMemberV1, PublicNominalKindV1};

mod decode;
mod validation;
pub use decode::*;
pub use validation::NominalDispatchDeclarationError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalDispatchOrderV1 {
    NonVirtual,
    Class {
        slots: Vec<PersistentDispatchSlotId>,
    },
    Interface {
        parents: Vec<SignatureTypeKey>,
        members: Vec<InterfaceSourceMemberV1>,
    },
}

impl NominalDispatchOrderV1 {
    pub fn empty(kind: PublicNominalKindV1) -> Self {
        match kind {
            PublicNominalKindV1::Class | PublicNominalKindV1::Object => {
                Self::Class { slots: Vec::new() }
            }
            PublicNominalKindV1::Interface => Self::Interface {
                parents: Vec::new(),
                members: Vec::new(),
            },
            PublicNominalKindV1::Struct | PublicNominalKindV1::Enum => Self::NonVirtual,
        }
    }

    pub fn declared_slots(&self) -> impl Iterator<Item = PersistentDispatchSlotId> + '_ {
        let (class, interface): (&[_], &[_]) = match self {
            Self::NonVirtual => (&[], &[]),
            Self::Class { slots } => (slots, &[]),
            Self::Interface { members, .. } => (&[], members),
        };
        class
            .iter()
            .copied()
            .chain(interface.iter().map(InterfaceSourceMemberV1::slot))
    }

    pub(super) fn validate_kind(
        &self,
        kind: PublicNominalKindV1,
    ) -> Result<(), NominalDispatchOrderError> {
        let valid = matches!(
            (self, kind),
            (
                Self::NonVirtual,
                PublicNominalKindV1::Struct | PublicNominalKindV1::Enum
            ) | (
                Self::Class { .. },
                PublicNominalKindV1::Class | PublicNominalKindV1::Object
            ) | (Self::Interface { .. }, PublicNominalKindV1::Interface)
        );
        if !valid {
            return Err(NominalDispatchOrderError::Kind);
        }
        self.validate_structure()
    }

    fn validate_structure(&self) -> Result<(), NominalDispatchOrderError> {
        let mut slots = BTreeSet::new();
        for slot in self.declared_slots() {
            if !slots.insert(slot) {
                return Err(NominalDispatchOrderError::DuplicateSlot(slot));
            }
        }
        if let Self::Interface { parents, members } = self {
            if parents.iter().collect::<BTreeSet<_>>().len() != parents.len() {
                return Err(NominalDispatchOrderError::DuplicateParent);
            }
            for member in members {
                if member
                    .overrides()
                    .values()
                    .binary_search(&member.slot())
                    .is_ok()
                {
                    return Err(NominalDispatchOrderError::SelfOverride(member.slot()));
                }
            }
        }
        Ok(())
    }
}

impl WireEncode for NominalDispatchOrderV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NonVirtual => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::Class { slots } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                sequence(encoder, slots)
            }
            Self::Interface { parents, members } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(3)?;
                encoder.field(1)?;
                sequence(encoder, parents)?;
                encoder.field(2)?;
                sequence(encoder, members)
            }
        }
    }
}

fn sequence<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominalDispatchOrderError {
    Kind,
    DuplicateSlot(PersistentDispatchSlotId),
    DuplicateParent,
    SelfOverride(PersistentDispatchSlotId),
}
impl std::fmt::Display for NominalDispatchOrderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid nominal dispatch declaration order: {self:?}")
    }
}
impl std::error::Error for NominalDispatchOrderError {}
