use scoop_identity::{
    CanonicalIdentifier, CanonicalScoopAbiFunctionSignature, ConeCoordinate, ConeIdentity,
    DeclarationScope, DefinitionOwnerChain, DependencyCallableDeclarationId, Effect, ExactTypeKey,
    GcEffect, PackagePath, PersistentExactTypeId, PersistentFunctionId, ScoopAbiReturn,
    SourceDeclarationKey, SourceDeclarationSite, StrongCallableDefinitionOwner,
};
use scoop_lir::{
    CallingConvention, CanonicalLirFoundation, CrossConeLirBridgeSectionV1,
    DependencyExternalCallableRootPlanV1, LirTargetProfile, OdrFreeLirFoundation,
    SelectedDependencyLirCallableV1,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;
use crate::link_object::native_requirements::tests::core_closure;
use crate::link_object::strong_relocation_closure::tests::{
    verified_member_with_undefined, verified_member_without_relocations,
};
use crate::link_object::symbol_verification::tests::fixture_for_producer;
use crate::link_object::undefined_requirements::tests::empty_bridge_plan;
use crate::link_object::{
    empty_code_link_object_member_set_for_test,
    finalize_partitioned_undefined_symbol_requirements_v1,
    finalize_undefined_symbol_requirements_v1, seal_builtin_object_external_requirements_v1,
    verify_current_cone_undefined_requirements_v1, verify_runtime_and_eh_requirements_v1,
    verify_source_external_requirements_after_cross_cone_v1,
    without_generated_bridge_semantics_for_test,
};

struct CallableFixture {
    provider: ConeIdentity,
    declaration: DependencyCallableDeclarationId,
    target: StrongCallableDefinitionOwner,
    abi: CanonicalScoopAbiFunctionSignature,
}

impl CallableFixture {
    fn new(provider_name: &str, callable_name: &str) -> Self {
        let provider = ConeCoordinate::new("test", provider_name, "1.0.0")
            .unwrap()
            .identity()
            .unwrap();
        let function =
            PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
                SourceDeclarationSite::new(
                    provider,
                    PackagePath::root(),
                    DefinitionOwnerChain::top_level(),
                    DeclarationScope::ConeWide,
                )
                .unwrap(),
                CanonicalIdentifier::new(callable_name).unwrap(),
                0,
                None,
                Vec::new(),
            ))
            .unwrap();
        let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ))
        .unwrap();
        Self {
            provider,
            declaration: DependencyCallableDeclarationId::Function(function),
            target: StrongCallableDefinitionOwner::Function(function),
            abi: CanonicalScoopAbiFunctionSignature::new(
                scoop_identity::ExactCallableSignature::new(
                    Effect::Ordinary,
                    None,
                    Vec::new(),
                    unit,
                ),
                Vec::new(),
                ScoopAbiReturn::unit_void(),
                GcEffect::NoGc,
            )
            .unwrap(),
        }
    }

    fn selected(&self) -> SelectedDependencyLirCallableV1 {
        SelectedDependencyLirCallableV1::new(
            self.provider,
            self.declaration,
            self.target,
            self.abi.clone(),
            CallingConvention::Cdecl,
            DependencyExternalCallableRootPlanV1::NoGc,
        )
        .unwrap()
    }
}

fn bridge(
    consumer: ConeIdentity,
    selected: Vec<SelectedDependencyLirCallableV1>,
) -> CrossConeLirBridgeSectionV1 {
    let foundation =
        OdrFreeLirFoundation::try_new(consumer, CanonicalLirFoundation::empty()).unwrap();
    CrossConeLirBridgeSectionV1::try_new(&foundation, Vec::new(), selected).unwrap()
}

#[test]
fn semantic_imports_are_a_canonical_projection_of_lir_selections() {
    let consumer = ConeIdentity::SINGLE_FILE;
    let first = CallableFixture::new("z-provider", "first");
    let second = CallableFixture::new("a-provider", "second");
    let lir = bridge(consumer, vec![first.selected(), second.selected()]);

    let semantic = CrossConeLinkSemanticImportSetV1::from_lir_bridge(&lir).unwrap();
    let mut expected = vec![
        (first.provider, first.target),
        (second.provider, second.target),
    ];
    expected.sort_unstable();

    assert_eq!(semantic.consumer(), consumer);
    assert_eq!(
        semantic
            .imports()
            .iter()
            .map(|import| (import.provider(), import.target()))
            .collect::<Vec<_>>(),
        expected
    );
    for import in semantic.imports() {
        let selected = lir
            .selected()
            .iter()
            .find(|selected| {
                selected.provider() == import.provider()
                    && selected.bridge().target() == import.target()
            })
            .unwrap();
        assert_eq!(import.abi_signature(), selected.bridge().abi_signature());
        assert_eq!(
            import.expected_symbol(),
            selected.bridge().expected_symbol()
        );
        assert_eq!(
            import.required_definition(),
            selected.bridge().required_definition()
        );
    }
}

#[test]
fn classifier_claims_only_selected_dependency_relocations() {
    let consumer = ConeIdentity::SINGLE_FILE;
    let callable = CallableFixture::new("dependency-provider", "dependencyCall");
    let lir = bridge(consumer, vec![callable.selected()]);
    let semantic = CrossConeLinkSemanticImportSetV1::from_lir_bridge(&lir).unwrap();
    let physical_symbol = LirTargetProfile::DARWIN_AARCH64
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(semantic.imports()[0].expected_symbol().symbol().as_str())
        .into_bytes();
    let object = fixture_for_producer(consumer, "dependencyCaller");
    let member = verified_member_with_undefined(&object, &physical_symbol);

    let verified =
        verify_cross_cone_strong_requirements_v1(core_closure(consumer, member), &lir).unwrap();

    assert_eq!(verified.producer(), consumer);
    assert_eq!(verified.semantic_imports(), &semantic);
    assert_eq!(verified.requirements().len(), 1);
    assert_eq!(verified.requirements()[0].import_index(), 0);
    assert_eq!(
        verified.requirements()[0].use_site().symbol(),
        physical_symbol
    );
    assert!(verified.remaining_external_candidates().is_empty());
}

#[test]
fn classifier_preserves_unmatched_candidates_and_rejects_unused_imports() {
    let consumer = ConeIdentity::SINGLE_FILE;
    let object = fixture_for_producer(consumer, "unmatchedCaller");
    let member = verified_member_with_undefined(&object, b"_unrelated");
    let empty = bridge(consumer, Vec::new());
    let verified =
        verify_cross_cone_strong_requirements_v1(core_closure(consumer, member), &empty).unwrap();
    assert!(verified.requirements().is_empty());
    assert_eq!(verified.remaining_external_candidates().len(), 1);

    let callable = CallableFixture::new("unused-provider", "unusedCall");
    let selected = bridge(consumer, vec![callable.selected()]);
    let object = fixture_for_producer(consumer, "noDependencyCall");
    let member = verified_member_without_relocations(&object);
    assert!(matches!(
        verify_cross_cone_strong_requirements_v1(core_closure(consumer, member), &selected),
        Err(CrossConeStrongRequirementValidationError::UnusedImport { .. })
    ));
}

#[test]
fn classifier_rejects_a_bridge_for_another_consumer() {
    let consumer = ConeIdentity::SINGLE_FILE;
    let other = ConeCoordinate::new("test", "other-consumer", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let object = fixture_for_producer(consumer, "wrongConsumer");
    let member = verified_member_without_relocations(&object);

    assert_eq!(
        verify_cross_cone_strong_requirements_v1(
            core_closure(consumer, member),
            &bridge(other, Vec::new()),
        ),
        Err(
            CrossConeStrongRequirementValidationError::ConsumerMismatch {
                object: consumer,
                bridge: other,
            }
        )
    );
}

#[test]
fn finalizer_keeps_legacy_and_cross_cone_uses_disjoint_and_complete() {
    let consumer = ConeIdentity::SINGLE_FILE;
    let callable = CallableFixture::new("partition-provider", "partitionCall");
    let lir = bridge(consumer, vec![callable.selected()]);
    let semantic = CrossConeLinkSemanticImportSetV1::from_lir_bridge(&lir).unwrap();
    let physical_symbol = LirTargetProfile::DARWIN_AARCH64
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(semantic.imports()[0].expected_symbol().symbol().as_str())
        .into_bytes();
    let object = fixture_for_producer(consumer, "partitionCaller");
    let member = verified_member_with_undefined(&object, &physical_symbol);
    let core = core_closure(consumer, member);
    let strong = core.strong_closure().clone();
    let current =
        verify_current_cone_undefined_requirements_v1(strong.clone(), empty_bridge_plan(consumer))
            .unwrap();
    let cross = verify_cross_cone_strong_requirements_v1(core, &lir).unwrap();
    let source = verify_source_external_requirements_after_cross_cone_v1(
        cross,
        crate::link_object::native_requirements::tests::native_surface(
            consumer,
            Vec::new(),
            Vec::new(),
        ),
    )
    .unwrap();
    let runtime = verify_runtime_and_eh_requirements_v1(
        source,
        scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    )
    .unwrap();
    let external = seal_builtin_object_external_requirements_v1(
        without_generated_bridge_semantics_for_test(runtime),
    )
    .unwrap();

    assert!(matches!(
        finalize_undefined_symbol_requirements_v1(current.clone(), external.clone()),
        Err(
            crate::UndefinedSymbolRequirementFinalizationError::CrossConeRequirementsRequirePartitionedFinalizer {
                imports: 1,
                requirements: 1,
            }
        )
    ));
    let partitions =
        finalize_partitioned_undefined_symbol_requirements_v1(current, external).unwrap();
    assert!(partitions.legacy().requirements().is_empty());
    assert_eq!(partitions.cross_cone().requirements().len(), 1);
    assert_eq!(
        partitions.cross_cone().requirements()[0].use_site(),
        &CanonicalUndefinedRelocationUseV1::from(&strong.bindings()[0])
    );
}

#[test]
fn link_closure_wire_round_trips_only_against_the_rebuilt_projection() {
    let semantic = CrossConeLinkSemanticImportSetV1::from_lir_bridge(&bridge(
        ConeIdentity::SINGLE_FILE,
        vec![],
    ))
    .unwrap();
    let expected = CrossConeLinkClosureSectionV1::from_parts(
        semantic.clone(),
        Vec::new(),
        empty_code_link_object_member_set_for_test(),
    )
    .unwrap();
    let bytes = encode(&expected).unwrap();
    assert_eq!(
        bytes,
        vec![
            0xa3, 0x01, 0x80, 0x02, 0x80, 0x03, 0xa2, 0x01, 0x80, 0x02, 0x58, 0x20, 0x95, 0x9c,
            0xe3, 0x93, 0x6d, 0x4d, 0x3f, 0xa0, 0x4e, 0xaf, 0xd3, 0x98, 0x6b, 0x9d, 0x89, 0x06,
            0xe7, 0xe0, 0x4b, 0x62, 0xb6, 0x4b, 0x59, 0x8c, 0x4b, 0xa4, 0x4d, 0x64, 0x0a, 0x1f,
            0x29, 0x7b,
        ]
    );

    let decoded =
        decode_canonical::<DecodedCrossConeLinkClosureSectionV1>(&bytes, DecodeLimits::default())
            .unwrap();
    decoded
        .validate_semantic_imports_against(&semantic)
        .unwrap();
    assert_eq!(decoded.validate_against(&expected).unwrap(), expected);

    let mut changed = expected.clone();
    changed.object_coverage.relocation_use_set_digest.0[0] ^= 0xff;
    let decoded =
        decode_canonical::<DecodedCrossConeLinkClosureSectionV1>(&bytes, DecodeLimits::default())
            .unwrap();
    assert!(matches!(
        decoded.validate_against(&changed),
        Err(CrossConeLinkClosureSectionValidationError::ProjectionMismatch)
    ));
}

#[test]
fn link_closure_reader_rejects_non_v1_product_shapes() {
    for bytes in [vec![0xa2], vec![0xa4]] {
        assert!(
            decode_canonical::<DecodedCrossConeLinkClosureSectionV1>(
                &bytes,
                DecodeLimits::default()
            )
            .is_err()
        );
    }
}
