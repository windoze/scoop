use super::*;
use crate::*;
use scoop_identity::{
    AccessorRole, CallableTemplateOrigin, CanonicalIdentifier, ConeCoordinate, ConeIdentity,
    CoreBuiltinNominal, DeclarationScope, DefinitionOrigin, DefinitionOwnerAtom,
    DefinitionOwnerChain, ExactTypeKey, LocalValueSelector, NominalDeclarationOwner,
    NormalizedSourcePath, PackagePath, PersistentConstructorId, PersistentDispatchSlotId,
    PersistentExactTypeId, PersistentObjectValueId, PersistentPropertyAccessorId,
    PersistentPropertyId, PersistentTypeId, PropertyAccessorKey, PropertyOwner, SignatureTypeKey,
    SourceContextKey, SourceDeclarationKey, SourceDeclarationSite, SourceIdentity,
    SourceNominalKind, SourceSpan, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment, SyntheticLocalRole,
};
use scoop_wire::{BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, WirePath};

mod authority;
mod defaults;
mod inheritance;
mod nested;
mod representations;
mod resources;
mod support;
use authority::{DefaultSite, Expected, UseKey};
use support::*;
