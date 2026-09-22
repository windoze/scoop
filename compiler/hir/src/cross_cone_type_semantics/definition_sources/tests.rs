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

#[test]
fn producer_collects_the_same_exact_recursive_origin_set_as_the_reader() {
    for fixture in [
        representations::fixture(2).0,
        nested::fixture(),
        defaults::fixture(),
    ] {
        let sources = fixture
            .inputs()
            .collect_definition_sources(
                &mut BudgetMeter::new(DecodeLimits::default()),
                &WirePath::root(),
            )
            .unwrap();
        assert_eq!(sources, fixture.declared().sources());
        let declared = CanonicalExportDefinitionSourcesV1::try_new(sources).unwrap();
        fixture
            .validate(&declared, DecodeLimits::default())
            .unwrap();
        for limits in [
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_leaf_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(
                fixture
                    .inputs()
                    .collect_definition_sources(&mut BudgetMeter::new(limits), &WirePath::root())
                    .is_err()
            );
        }
    }
}
