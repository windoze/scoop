use super::*;
use crate::*;
use scoop_identity::{
    CanonicalIdentifier, ConeCoordinate, ConeIdentity, CoreBuiltinNominal, DeclarationScope,
    DefinitionOrigin, DefinitionOwnerAtom, DefinitionOwnerChain, EnumVariantIdentityKey,
    FieldIdentityKey, NonEmptyVec, PackagePath, PersistentTypeId, SignatureTypeKey,
    SourceContextKey, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, SourceSpan,
};
use scoop_wire::{BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, WirePath};

mod fixtures;
mod join;
mod mismatches;
mod resources;
mod support;
use support::*;
