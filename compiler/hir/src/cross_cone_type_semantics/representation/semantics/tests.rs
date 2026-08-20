use super::*;
use crate::*;
use scoop_identity::{
    CanonicalIdentifier, ConeCoordinate, ConeIdentity, CoreBuiltinNominal, DeclarationScope,
    DefinitionOrigin, DefinitionOwnerAtom, DefinitionOwnerChain, EnumVariantIdentityKey,
    FieldIdentityKey, PackagePath, PersistentTypeId, SignatureTypeKey, SourceContextKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, SourceSpan,
};
use scoop_wire::WirePath;

mod fixtures;
mod join;
mod mismatches;

mod support;
use support::*;
