use scoop_identity::*;
use scoop_lir::*;
use scoop_wire::encode_runtime;

use super::*;
use crate::link_object::native_requirements::tests::native_surface;
use crate::link_object::strong_relocation_closure::tests::{
    verified_member_with_undefined, verified_member_without_relocations,
};
use crate::link_object::symbol_verification::tests::fixture_for_producer;
use crate::link_object::undefined_requirements::tests::empty_bridge_plan;
use crate::link_object::*;

struct OldCallable {
    provider: ConeIdentity,
    target: StrongCallableDefinitionOwner,
    bridge: CrossConeLirBridgeSectionV1,
    symbol: Vec<u8>,
}

fn old_callable(consumer: ConeIdentity) -> OldCallable {
    let provider = ConeCoordinate::new("test", "partition-callable", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let function = PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("oldCall").unwrap(),
        0,
        None,
        vec![],
    ))
    .unwrap();
    let target = StrongCallableDefinitionOwner::Function(function);
    let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap();
    let selected = SelectedDependencyLirCallableV1::new(
        provider,
        DependencyCallableDeclarationId::Function(function),
        target,
        CanonicalScoopAbiFunctionSignature::new(
            ExactCallableSignature::new(Effect::Ordinary, None, vec![], unit),
            vec![],
            ScoopAbiReturn::unit_void(),
            scoop_identity::GcEffect::NoGc,
        )
        .unwrap(),
        scoop_lir::CallingConvention::Cdecl,
        ExternalCallableRootPlan::NoGc,
    )
    .unwrap();
    let foundation = ConeLirFoundation::try_new(consumer, CanonicalLirFoundation::empty()).unwrap();
    let bridge = CrossConeLirBridgeSectionV1::try_new(&foundation, vec![], vec![selected]).unwrap();
    let imports = CrossConeLinkSemanticImportSetV1::from_lir_bridge(&bridge).unwrap();
    let symbol = TARGET
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(imports.imports()[0].expected_symbol().symbol().as_str())
        .into_bytes();
    OldCallable {
        provider,
        target,
        bridge,
        symbol,
    }
}

#[test]
fn layout_finalizer_keeps_disjoint_partitions_and_encodes_the_original_definition() {
    let consumer_id = ConeIdentity::SINGLE_FILE;
    let provider = Provider::new();
    let layout = provider.consumer(consumer_id);
    let shape_import = &layout.selected().physical_imports().records()[0];
    let shape_symbol = TARGET
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(shape_import.expected_symbol().symbol().as_str())
        .into_bytes();
    let callable = old_callable(consumer_id);
    let fixtures = ["oldPartition", "shapePartition", "runtimePartition"]
        .map(|name| fixture_for_producer(consumer_id, name));
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_with_undefined(&fixtures[0], &callable.symbol),
        verified_member_with_undefined(&fixtures[1], &shape_symbol),
        verified_member_with_undefined(&fixtures[2], b"_scoop_runtime_alloc_slow"),
    ])
    .unwrap();
    let current = verify_current_cone_undefined_requirements_v1(
        strong.clone(),
        empty_bridge_plan(consumer_id),
    )
    .unwrap();
    let provider_object = fixture_for_producer(callable.provider, "oldCall");
    let provider_strong = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&provider_object),
    ])
    .unwrap();
    let owners =
        CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&provider_strong)
            .unwrap();
    let old = verify_cross_cone_strong_requirements_v1(
        TARGET,
        strong.clone(),
        &[owners],
        &callable.bridge,
    )
    .unwrap();
    let shape = verify_external_shape_requirements_v1(
        &old,
        layout.selected().consumer(),
        layout.selected().physical_imports(),
    )
    .unwrap();
    let source = verify_source_external_requirements_after_external_shape_v1(
        &shape,
        native_surface(consumer_id, vec![], vec![]),
    )
    .unwrap();
    let runtime =
        verify_runtime_and_eh_requirements_v1(source, scoop_target_data::DARWIN_AARCH64_LLVM_22_1)
            .unwrap();
    let external = without_target_support_for_test(runtime);

    let partitions = finalize_layout_partitioned_undefined_symbol_requirements_v1(
        current,
        external.clone(),
        &shape,
    )
    .unwrap();
    assert_eq!(partitions.legacy().requirements().len(), 1);
    assert_eq!(partitions.cross_cone().requirements().len(), 1);
    assert_eq!(partitions.external_shape().len(), 1);
    let requirements = VerifiedObjectDefinitionRequirementSetV1::from(partitions.clone());

    let shape_requirement = partitions.external_shape()[0].use_site();
    let truncated = VerifiedObjectDefinitionRequirementSetV1::from(partitions.legacy().clone());
    assert!(truncated.requirement_for(shape_requirement).is_none());
    let requirement = requirements.requirement_for(shape_requirement).unwrap();
    assert_eq!(
        requirement,
        CanonicalObjectDefinitionRequirementV1::DependencyShapeStrong {
            provider: provider.foundation.producer(),
            subject: ExternalStrongShapeSubjectV1::Layout(provider.layout),
        }
    );
    let mut expected = 1_u32.to_le_bytes().to_vec();
    expected.extend_from_slice(&5_u32.to_le_bytes());
    expected.extend_from_slice(provider.layout.as_array());
    expected.extend_from_slice(&5_u32.to_le_bytes());
    assert_eq!(encode_runtime(&requirement).unwrap(), expected);

    let old_requirement = requirements
        .requirement_for(partitions.cross_cone().requirements()[0].use_site())
        .unwrap();
    assert_eq!(
        old_requirement,
        CanonicalObjectDefinitionRequirementV1::DependencyStrong {
            provider: callable.provider,
            target: callable.target,
        }
    );
}
