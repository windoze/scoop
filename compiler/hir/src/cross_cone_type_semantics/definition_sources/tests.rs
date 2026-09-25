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
use scoop_wire::WirePath;

mod authority;
mod defaults;
mod inheritance;
mod nested;
mod representations;

mod support;
use authority::{DefaultSite, Expected, UseKey};
use support::*;

#[test]
fn producer_collects_the_same_exact_recursive_origin_set_as_the_reader() {
    for fixture in [
        representations::fixture(2).0,
        nested::fixture(),
        defaults::fixture(),
    ] {
        let sources = fixture
            .inputs()
            .collect_definition_sources(&WirePath::root())
            .unwrap();
        assert_eq!(sources, fixture.declared().sources());
        let declared = CanonicalExportDefinitionSourcesV1::try_new(sources).unwrap();
        fixture.validate(&declared).unwrap();
    }
}
