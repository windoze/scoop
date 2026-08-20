use super::*;
use scoop_identity::{
    AccessorRole, CanonicalIdentifier, ConeCoordinate, ConeIdentity, DeclarationScope,
    DecodedPersistentId, DefinitionOwnerAtom, DefinitionOwnerChain, DispatchSlotKey,
    EnumVariantIdentityKey, ExactTypeKey, PackagePath, PersistentConstructorId,
    PersistentDispatchSlotId, PersistentEnumVariantId, PersistentExactTypeId, PersistentFunctionId,
    PersistentIdResolver, PersistentObjectValueId, PersistentPropertyAccessorId,
    PersistentPropertyId, PersistentTypeId, PropertyAccessorKey, PropertyOwner,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{
    Encoder, WireDecode, WireEncode, WireErrorKind, WirePath, decode_canonical, encode,
};

mod canonical;
mod fixture;
mod malformed;
mod resolver;
mod resources;
mod support;
mod wire;

use fixture::Fixture;
use resolver::{Family, Resolver};
use support::*;
