use super::*;
use crate::*;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DecodedPersistentId, EnumVariantFieldKey,
    EnumVariantIdentityKey, FieldIdentityKey, NonEmptyVec, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentFieldId, PersistentGenericTypeId, PersistentIdResolver,
    PersistentKeyResolver, PersistentSourceContextId, PersistentTypeId, SignatureTypeKey,
    SourceContextKey, SourceDeclarationKey, SourceNominalKind,
};
use scoop_wire::{
    BudgetMeter, DecodeLimits, Encoder, ResourceKind, WireDecode, WireEncode, WireErrorKind,
    WirePath, decode_canonical, encode,
};
use std::sync::Arc;

mod budget;
mod canonical;
mod fixtures;
mod resolver;
mod support;
mod wire;
use resolver::Counting;
use support::*;
