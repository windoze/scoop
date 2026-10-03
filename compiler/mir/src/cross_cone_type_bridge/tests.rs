use super::*;
use scoop_identity::{
    CborIdentityRecord, EnumVariantFieldKey, EnumVariantFieldSelector, EnumVariantIdentityKey,
    FieldIdentityKey, SourceDeclarationKey, SourceNominalKind,
};
use scoop_wire::{decode_canonical, encode};

mod objects;
mod release;
pub(in crate::cross_cone_type_bridge) mod support;
mod validation;
mod wire;
use support::*;
