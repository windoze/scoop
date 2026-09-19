use super::*;
use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerAtom,
    DefinitionOwnerChain, DispatchSlotKey, Effect, InitializationCallableRole,
    InitializationUnitKey, PackagePath, PendingIdentityValidation, PropertyAccessorKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

mod budget;
mod builders;
mod generated;
mod source;
mod support;
mod wire;
use builders::*;
use support::*;
