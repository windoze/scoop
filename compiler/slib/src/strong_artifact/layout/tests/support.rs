use scoop_hir::*;
use scoop_identity::{CborIdentityRecord, ConeIdentity, ExactTypeKey};
use scoop_mir::*;
use scoop_wire::{DecodeLimits, decode_canonical};

use crate::ConeRecord;

pub(super) fn hir_foundation_and_production(
    cone: &ConeRecord,
) -> (OdrFreeHirFoundation, CoreBootstrapInterfaceSectionV1) {
    let mut foundation = CanonicalHirFoundation::empty();
    foundation
        .set_types(vec![
            scoop_identity::CoreBuiltinNominal::Unit.identity_record(),
            scoop_identity::CoreBuiltinNominal::Any.identity_record(),
        ])
        .unwrap();
    let foundation = OdrFreeHirFoundation::try_new(foundation).unwrap();
    let production = decode_canonical::<DecodedCoreBootstrapInterfaceSectionV1>(
        &[0xa3, 0x02, 0xa1, 0x00, 0x01, 0x03, 0x80, 0x04, 0x80],
        DecodeLimits::default(),
    )
    .unwrap()
    .validate_against_strong_foundation(cone.identity(), &foundation)
    .unwrap();
    (foundation, production)
}

pub(super) fn empty_hir_interface() -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(vec![]).unwrap(),
        CanonicalNominalInterfacesV1::try_new(vec![]).unwrap(),
        CanonicalCallableInterfacesV1::try_new(vec![]).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(vec![]).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(vec![]).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(vec![]).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(vec![]).unwrap(),
        CanonicalExportConstValuesV1::try_new(vec![]).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(vec![]).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(vec![]).unwrap(),
    )
}

pub(super) fn empty_hir_type_semantics() -> CrossConeTypeSemanticsSectionV1 {
    CrossConeTypeSemanticsSectionV1::new(
        CanonicalExactTypeFactsV1::try_new(vec![]).unwrap(),
        CanonicalNominalRepresentationSupportV1::try_new(vec![]).unwrap(),
        CanonicalNominalInheritanceInterfacesV1::try_new(vec![]).unwrap(),
        CanonicalProtectedDeclarationInterfacesV1::try_new(vec![]).unwrap(),
        CanonicalProtectedCallableSourceInterfacesV1::try_new(vec![]).unwrap(),
        CanonicalProtectedDefaultTemplatesV1::try_new(vec![]).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(vec![]).unwrap(),
        CanonicalSelectedExternalTypeUsesV1::try_new(vec![]).unwrap(),
    )
}

pub(super) fn mir_sections(
    provider: ConeIdentity,
) -> (
    OdrFreeMirFoundation,
    CoreBootstrapBridgeSectionV1,
    CrossConeMirBridgeSectionV1,
) {
    let mut foundation = CanonicalMirFoundation::empty();
    foundation
        .set_exact_types(vec![
            CborIdentityRecord::from_key(ExactTypeKey::Nominal(
                scoop_identity::CoreBuiltinNominal::Unit
                    .identity_record()
                    .id(),
            ))
            .unwrap(),
        ])
        .unwrap();
    let foundation = OdrFreeMirFoundation::try_new(foundation).unwrap();
    let ordinary =
        CrossConeMirBridgeSectionV1::try_new(provider, &foundation, vec![], vec![]).unwrap();
    let production = CoreBootstrapBridgeSectionV1::try_new(
        provider,
        EntryMirBridgeBranchV1::Library,
        StrongCallableBridgeSurfaceV1::from_odr_free_foundation(&foundation),
    )
    .unwrap();
    (foundation, production, ordinary)
}
