use scoop_identity::{
    CanonicalIdentifier, CanonicalScoopAbiFunctionSignature, ConeCoordinate, ConeIdentity,
    DeclarationScope, DefinitionOwnerChain, DependencyCallableDeclarationId, Effect, ExactTypeKey,
    GcEffect, PackagePath, PersistentExactTypeId, PersistentFunctionId, ScoopAbiReturn,
    SourceDeclarationKey, SourceDeclarationSite, StrongCallableDefinitionOwner,
};
use scoop_lir::{
    CallingConvention, CanonicalLirFoundation, ConeLirFoundation, CrossConeLirBridgeSectionV1,
    ExternalCallableRootPlan, SelectedDependencyLirCallableV1,
};

use super::*;
use crate::link_object::native_requirements::tests::dependency_closure;
use crate::link_object::strong_relocation_closure::tests::{
    verified_member_with_undefined, verified_member_without_relocations,
};
use crate::link_object::symbol_verification::tests::fixture_for_producer;
use crate::link_object::undefined_requirements::tests::empty_bridge_plan;
use crate::link_object::{
    CanonicalObjectDefinitionRequirementV1, VerifiedObjectDefinitionRequirementSetV1,
    finalize_partitioned_undefined_symbol_requirements_v1,
    verify_current_cone_undefined_requirements_v1, verify_runtime_and_eh_requirements_v1,
    verify_source_external_requirements_v1,
};

struct CallableFixture {
    provider: ConeIdentity,
    name: String,
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
            name: callable_name.to_owned(),
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

    fn owners(&self) -> crate::CanonicalDefinedLinkSymbolOwnerSetV1 {
        let fixture = fixture_for_producer(self.provider, &self.name);
        let strong = crate::verify_current_cone_strong_relocation_closure_v1(vec![
            verified_member_without_relocations(&fixture),
        ])
        .unwrap();
        crate::CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&strong).unwrap()
    }

    fn selected(&self) -> SelectedDependencyLirCallableV1 {
        SelectedDependencyLirCallableV1::new(
            self.provider,
            self.declaration,
            self.target,
            self.abi.clone(),
            CallingConvention::Cdecl,
            ExternalCallableRootPlan::NoGc,
        )
        .unwrap()
    }
}

fn bridge(
    consumer: ConeIdentity,
    selected: Vec<SelectedDependencyLirCallableV1>,
) -> CrossConeLirBridgeSectionV1 {
    let foundation = ConeLirFoundation::try_new(consumer, CanonicalLirFoundation::empty()).unwrap();
    CrossConeLirBridgeSectionV1::try_new(&foundation, Vec::new(), selected).unwrap()
}

fn classify_callables(
    base: VerifiedCrossConeStrongRequirementClosureV1,
    bridge: &CrossConeLirBridgeSectionV1,
    owners: &[crate::CanonicalDefinedLinkSymbolOwnerSetV1],
) -> Result<VerifiedCrossConeStrongRequirementClosureV1, CrossConeStrongRequirementValidationError>
{
    verify_cross_cone_strong_requirements_v1(
        base.target(),
        base.strong_closure().clone(),
        owners,
        bridge,
    )
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
    let physical_symbol = scoop_target_data::DARWIN_AARCH64
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(semantic.imports()[0].expected_symbol().symbol().as_str())
        .into_bytes();
    let object = fixture_for_producer(consumer, "dependencyCaller");
    let member = verified_member_with_undefined(&object, &physical_symbol);

    let verified =
        classify_callables(dependency_closure(member), &lir, &[callable.owners()]).unwrap();

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
fn classifier_preserves_unmatched_candidates_and_metadata_only_imports() {
    let consumer = ConeIdentity::SINGLE_FILE;
    let object = fixture_for_producer(consumer, "unmatchedCaller");
    let member = verified_member_with_undefined(&object, b"_unrelated");
    let empty = bridge(consumer, Vec::new());
    let verified = classify_callables(dependency_closure(member), &empty, &[]).unwrap();
    assert!(verified.requirements().is_empty());
    assert_eq!(verified.remaining_external_candidates().len(), 1);

    let callable = CallableFixture::new("unused-provider", "unusedCall");
    let selected = bridge(consumer, vec![callable.selected()]);
    let object = fixture_for_producer(consumer, "noDependencyCall");
    let member = verified_member_without_relocations(&object);
    let classified =
        classify_callables(dependency_closure(member), &selected, &[callable.owners()]).unwrap();
    assert_eq!(classified.semantic_imports().imports().len(), 1);
    assert!(classified.requirements().is_empty());
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
        classify_callables(dependency_closure(member), &bridge(other, Vec::new()), &[],),
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
    let physical_symbol = scoop_target_data::DARWIN_AARCH64
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(semantic.imports()[0].expected_symbol().symbol().as_str())
        .into_bytes();
    let object = fixture_for_producer(consumer, "partitionCaller");
    let member = verified_member_with_undefined(&object, &physical_symbol);
    let core = dependency_closure(member);
    let strong = core.strong_closure().clone();
    let current =
        verify_current_cone_undefined_requirements_v1(strong.clone(), empty_bridge_plan(consumer))
            .unwrap();
    let cross = classify_callables(core, &lir, &[callable.owners()]).unwrap();
    let source = verify_source_external_requirements_v1(
        cross,
        crate::link_object::native_requirements::tests::native_surface(
            consumer,
            Vec::new(),
            Vec::new(),
        ),
    )
    .unwrap();
    let external =
        verify_runtime_and_eh_requirements_v1(source, scoop_target_data::DARWIN_AARCH64_LLVM_22_1)
            .unwrap();

    let partitions = finalize_partitioned_undefined_symbol_requirements_v1(current, external);
    assert!(partitions.legacy().requirements().is_empty());
    assert_eq!(partitions.cross_cone().requirements().len(), 1);
    let object_requirements = VerifiedObjectDefinitionRequirementSetV1::from(partitions.clone());
    assert_eq!(
        object_requirements.requirement_for(partitions.cross_cone().requirements()[0].use_site()),
        Some(CanonicalObjectDefinitionRequirementV1::DependencyStrong {
            provider: callable.provider,
            target: callable.target,
        })
    );
    assert_eq!(
        partitions.cross_cone().requirements()[0].use_site(),
        &CanonicalUndefinedRelocationUseV1::from(&strong.bindings()[0])
    );
}

#[test]
fn callable_provider_directory_rejects_missing_duplicate_and_self_entries() {
    let consumer = ConeIdentity::CORE;
    let callable = CallableFixture::new("provider", "target");
    let selected = bridge(consumer, vec![callable.selected()]);
    let object = fixture_for_producer(consumer, "caller");
    let source = dependency_closure(verified_member_without_relocations(&object));
    assert!(matches!(classify_callables(source.clone(), &selected, &[]),
        Err(CrossConeStrongRequirementValidationError::MissingProvider { provider }) if provider == callable.provider));
    let owners = callable.owners();
    assert!(
        matches!(classify_callables(source.clone(), &selected, &[owners.clone(), owners.clone()]),
        Err(CrossConeStrongRequirementValidationError::DuplicateProvider { provider }) if provider == callable.provider)
    );
    let self_owners = crate::CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(
        source.strong_closure(),
    )
    .unwrap();
    assert!(
        matches!(classify_callables(source.clone(), &selected, &[self_owners]),
        Err(CrossConeStrongRequirementValidationError::SelfDependency { provider }) if provider == consumer)
    );
    let actual = classify_callables(source, &selected, &[owners]).unwrap();
    assert_eq!(
        actual.semantic_imports().imports()[0].provider(),
        callable.provider
    );
}
