use scoop_hir::{
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalExportConstValuesV1, CanonicalExportDefaultTemplatesV1,
    CanonicalExportDefinitionSourcesV1, CanonicalExternalHirReferencesV1, CanonicalHirFoundation,
    CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1, CanonicalPublicExportBindingsV1,
    CanonicalTypeAliasInterfacesV1, CrossConeHirInterfaceSectionV1, ExportBindingSourceV1,
    PublicExportBindingRecordV1,
};
use scoop_identity::{
    BindableEntity, BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate,
    ConeIdentity, CoreBuiltinNominal, DeclarationScope, DefinitionOwnerChain, ExportBindingKey,
    PackagePath, PersistentExportBindingId, PersistentFunctionId, SourceDeclarationKey,
    SourceDeclarationSite,
};
use scoop_wire::encode;

use super::*;
use crate::{
    ConeKind, ConeRecord, ConeSourceForm, HirFingerprint,
    cross_cone_compile_decode::tests::{
        cross_cone_artifact_for, cross_cone_artifact_for_with_hir_foundation,
        empty_cross_cone_hir_interface,
    },
    strong_compile_decode::tests::{cone_named, open_graph},
};

#[test]
fn profile_graph_assigns_direct_and_support_roles_after_closure_validation() {
    let core_bytes = artifact(core_cone(), Vec::new());
    let core = decode(&core_bytes);
    let terminal_bytes = artifact(cone_named("terminal"), vec![core.dependency_record()]);
    let terminal = decode(&terminal_bytes);
    let facade_bytes = artifact(cone_named("facade"), vec![terminal.dependency_record()]);

    let core = decode(&core_bytes);
    let terminal = decode(&terminal_bytes);
    let facade = decode(&facade_bytes);
    let mut direct = vec![ConeIdentity::CORE, facade.identity()];
    direct.sort_unstable();
    let closure = DecodedCrossConeClosure::new(
        cone_named("current").identity(),
        target(),
        direct,
        vec![core, terminal, facade],
    )
    .validate_profile_graph()
    .unwrap();

    assert_eq!(closure.current(), cone_named("current").identity());
    assert_eq!(closure.target_selection(), target());
    assert_eq!(closure.dependency_first().count(), 3);
    assert_eq!(
        closure.role(ConeIdentity::CORE),
        Some(CrossConeProviderRole::Direct)
    );
    assert_eq!(
        closure.role(cone_named("facade").identity()),
        Some(CrossConeProviderRole::Direct)
    );
    assert_eq!(
        closure.role(cone_named("terminal").identity()),
        Some(CrossConeProviderRole::Support)
    );
    assert!(
        closure
            .artifact(cone_named("terminal").identity())
            .is_some()
    );
    assert_eq!(closure.role(cone_named("absent").identity()), None);
}

#[test]
fn core_current_has_the_only_valid_empty_provider_closure() {
    let closure =
        DecodedCrossConeClosure::new(ConeIdentity::CORE, target(), Vec::new(), Vec::new())
            .validate_profile_graph()
            .unwrap();
    assert_eq!(closure.current(), ConeIdentity::CORE);
    assert_eq!(closure.dependency_first().count(), 0);

    let validated = closure
        .validate_identities()
        .unwrap()
        .validate_foundation_structure()
        .unwrap()
        .resolve_hir_interfaces()
        .unwrap()
        .validate_hir_productions()
        .unwrap()
        .validate_internal_hir_closures()
        .unwrap()
        .validate_nominal_surfaces()
        .unwrap()
        .validate_property_surfaces()
        .unwrap()
        .validate_callable_surfaces()
        .unwrap()
        .validate_type_alias_surfaces()
        .unwrap()
        .validate_public_binding_routes()
        .unwrap();
    assert_eq!(validated.current(), ConeIdentity::CORE);
    assert_eq!(validated.dependency_first().count(), 0);

    let core_bytes = artifact(core_cone(), Vec::new());
    assert_eq!(
        DecodedCrossConeClosure::new(
            ConeIdentity::CORE,
            target(),
            vec![ConeIdentity::CORE],
            vec![decode(&core_bytes)],
        )
        .validate_profile_graph()
        .err(),
        Some(CrossConeClosureGraphError::CoreHasDependencyProviders)
    );
}

#[test]
fn nominal_authority_walks_only_the_provider_transitive_dependencies() {
    let dependencies = vec![vec![], vec![], vec![0], vec![1], vec![2, 3]];

    assert_eq!(
        surface_validation::transitive_positions_for_test(4, &dependencies),
        vec![0, 1, 2, 3]
    );
    assert_eq!(
        surface_validation::transitive_positions_for_test(2, &dependencies),
        vec![0]
    );
    assert_eq!(
        surface_validation::transitive_positions_for_test(3, &dependencies),
        vec![1]
    );
}

#[test]
fn hir_production_failure_is_attributed_to_the_exact_artifact() {
    let core_bytes = artifact(core_cone(), Vec::new());
    let mut direct = vec![ConeIdentity::CORE];
    direct.sort_unstable();

    assert!(matches!(
        DecodedCrossConeClosure::new(
            cone_named("current").identity(),
            target(),
            direct,
            vec![decode(&core_bytes)],
        )
        .validate_profile_graph()
        .unwrap()
        .validate_identities()
        .unwrap()
        .validate_foundation_structure()
        .unwrap()
        .resolve_hir_interfaces()
        .unwrap()
        .validate_hir_productions(),
        Err(CrossConeClosureHirProductionError::Artifact {
            identity: ConeIdentity::CORE,
            source: scoop_hir::CoreBootstrapInterfaceValidationError::MissingCoreInterface,
        })
    ));
}

#[test]
fn non_core_closure_requires_implicit_core_and_canonical_direct_set() {
    let dependency_bytes = artifact(cone_named("dependency"), Vec::new());
    let dependency = decode(&dependency_bytes);
    let identity = dependency.identity();
    assert_eq!(
        DecodedCrossConeClosure::new(
            cone_named("current").identity(),
            target(),
            vec![identity],
            vec![dependency],
        )
        .validate_profile_graph()
        .err(),
        Some(CrossConeClosureGraphError::MissingTrustedCore)
    );

    let core_bytes = artifact(core_cone(), Vec::new());
    assert!(matches!(
        DecodedCrossConeClosure::new(
            cone_named("current").identity(),
            target(),
            vec![ConeIdentity::CORE, ConeIdentity::CORE],
            vec![decode(&core_bytes)],
        )
        .validate_profile_graph(),
        Err(CrossConeClosureGraphError::NonCanonicalDirectProviders { .. })
    ));
}

#[test]
fn profile_graph_rejects_non_dependency_first_artifacts() {
    let core_bytes = artifact(core_cone(), Vec::new());
    let core = decode(&core_bytes);
    let dependent_bytes = artifact(cone_named("dependent"), vec![core.dependency_record()]);
    let dependent = decode(&dependent_bytes);
    let mut direct = vec![ConeIdentity::CORE, dependent.identity()];
    direct.sort_unstable();

    assert!(matches!(
        DecodedCrossConeClosure::new(
            cone_named("current").identity(),
            target(),
            direct,
            vec![dependent, decode(&core_bytes)],
        )
        .validate_profile_graph(),
        Err(CrossConeClosureGraphError::InvalidDependencyFirstOrder {
            dependency: ConeIdentity::CORE,
            ..
        })
    ));
}

#[test]
fn profile_graph_rejects_a_stale_dependency_fingerprint() {
    let core_bytes = artifact(core_cone(), Vec::new());
    let core = decode(&core_bytes);
    let actual = core.dependency_record();
    let stale = crate::DependencyRecord::new(
        actual.coordinate().clone(),
        HirFingerprint::from_array([9; 32]),
        actual.mir_fingerprint(),
        actual.lir_fingerprint(),
    )
    .unwrap();
    let dependent_bytes = artifact(cone_named("dependent"), vec![stale]);
    let dependent = decode(&dependent_bytes);
    let mut direct = vec![ConeIdentity::CORE, dependent.identity()];
    direct.sort_unstable();

    assert!(matches!(
        DecodedCrossConeClosure::new(
            cone_named("current").identity(),
            target(),
            direct,
            vec![decode(&core_bytes), dependent],
        )
        .validate_profile_graph(),
        Err(CrossConeClosureGraphError::StaleDependency {
            dependency: ConeIdentity::CORE,
            ..
        })
    ));
}

#[test]
fn profile_graph_rejects_an_unreachable_support_artifact() {
    let core_bytes = artifact(core_cone(), Vec::new());
    let direct_bytes = artifact(cone_named("direct"), Vec::new());
    let unused_bytes = artifact(cone_named("unused"), Vec::new());
    let direct_identity = decode(&direct_bytes).identity();
    let unused_identity = decode(&unused_bytes).identity();
    let mut direct = vec![ConeIdentity::CORE, direct_identity];
    direct.sort_unstable();

    assert_eq!(
        DecodedCrossConeClosure::new(
            cone_named("current").identity(),
            target(),
            direct,
            vec![
                decode(&core_bytes),
                decode(&unused_bytes),
                decode(&direct_bytes),
            ],
        )
        .validate_profile_graph()
        .err(),
        Some(CrossConeClosureGraphError::UnreachableSupport {
            identity: unused_identity,
        })
    );
}

#[test]
fn profile_graph_rejects_two_versions_of_one_coordinate_family() {
    let core_bytes = artifact(core_cone(), Vec::new());
    let first = ConeRecord::new(
        ConeCoordinate::new("test", "versioned", "1.0.0").unwrap(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap();
    let second = ConeRecord::new(
        ConeCoordinate::new("test", "versioned", "2.0.0").unwrap(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap();
    let first_bytes = artifact(first, Vec::new());
    let second_bytes = artifact(second, Vec::new());
    let mut direct = vec![
        ConeIdentity::CORE,
        decode(&first_bytes).identity(),
        decode(&second_bytes).identity(),
    ];
    direct.sort_unstable();

    assert!(matches!(
        DecodedCrossConeClosure::new(
            cone_named("current").identity(),
            target(),
            direct,
            vec![
                decode(&core_bytes),
                decode(&first_bytes),
                decode(&second_bytes),
            ],
        )
        .validate_profile_graph(),
        Err(CrossConeClosureGraphError::MultipleVersions { .. })
    ));
}

#[test]
fn identity_registration_resolves_reexport_targets_from_the_provider_closure() {
    let core_bytes = artifact(core_cone(), Vec::new());
    let core = decode(&core_bytes);

    let terminal_cone = cone_named("terminal");
    let function = function_record(terminal_cone.identity(), "target");
    let mut terminal_foundation = base_hir_foundation();
    terminal_foundation
        .set_functions(vec![function.clone()])
        .unwrap();
    let terminal_bytes = artifact_with_foundation(
        terminal_cone,
        vec![core.dependency_record()],
        &terminal_foundation,
    );
    let terminal = decode(&terminal_bytes);

    let facade_cone = cone_named("facade");
    let binding =
        CborIdentityRecord::<PersistentExportBindingId, _>::from_key(ExportBindingKey::new(
            facade_cone.identity(),
            PackagePath::root(),
            CanonicalIdentifier::new("forwarded").unwrap(),
            BindingTarget::function(function.key()).unwrap(),
        ))
        .unwrap();
    let mut facade_foundation = base_hir_foundation();
    facade_foundation
        .set_export_bindings(vec![binding])
        .unwrap();
    let facade_bytes = artifact_with_foundation(
        facade_cone,
        vec![terminal.dependency_record()],
        &facade_foundation,
    );
    let facade = decode(&facade_bytes);
    let facade_identity = facade.identity();

    let mut direct = vec![ConeIdentity::CORE, facade_identity];
    direct.sort_unstable();
    let closure = DecodedCrossConeClosure::new(
        cone_named("current").identity(),
        target(),
        direct,
        vec![decode(&core_bytes), decode(&terminal_bytes), facade],
    )
    .validate_profile_graph()
    .unwrap()
    .validate_identities()
    .unwrap();

    let (_, facade_identities) = closure.artifact(facade_identity).unwrap();
    assert_eq!(facade_identities.declared_identity_count(), 18);
    assert_eq!(closure.dependency_first().count(), 3);
}

#[test]
fn identity_registration_does_not_leak_authority_between_siblings() {
    let core_bytes = artifact(core_cone(), Vec::new());
    let core = decode(&core_bytes);

    let owner_cone = cone_named("owner");
    let function = function_record(owner_cone.identity(), "target");
    let mut owner_foundation = base_hir_foundation();
    owner_foundation
        .set_functions(vec![function.clone()])
        .unwrap();
    let owner_bytes = artifact_with_foundation(
        owner_cone,
        vec![core.dependency_record()],
        &owner_foundation,
    );
    let owner = decode(&owner_bytes);

    let sibling_cone = cone_named("sibling");
    let binding =
        CborIdentityRecord::<PersistentExportBindingId, _>::from_key(ExportBindingKey::new(
            sibling_cone.identity(),
            PackagePath::root(),
            CanonicalIdentifier::new("stolen").unwrap(),
            BindingTarget::function(function.key()).unwrap(),
        ))
        .unwrap();
    let mut sibling_foundation = base_hir_foundation();
    sibling_foundation
        .set_export_bindings(vec![binding])
        .unwrap();
    let sibling_bytes = artifact_with_foundation(
        sibling_cone,
        vec![core.dependency_record()],
        &sibling_foundation,
    );
    let sibling = decode(&sibling_bytes);
    let sibling_identity = sibling.identity();

    let mut direct = vec![ConeIdentity::CORE, owner.identity(), sibling_identity];
    direct.sort_unstable();
    assert!(matches!(
        DecodedCrossConeClosure::new(
            cone_named("current").identity(),
            target(),
            direct,
            vec![decode(&core_bytes), owner, sibling],
        )
        .validate_profile_graph()
        .unwrap()
        .validate_identities(),
        Err(CrossConeClosureIdentityError::Artifact { identity, .. })
            if identity == sibling_identity
    ));
}

#[test]
fn closure_foundations_validate_in_dependency_order() {
    let core_bytes = artifact(core_cone(), Vec::new());
    let core = decode(&core_bytes);
    let provider_bytes = artifact(cone_named("provider"), vec![core.dependency_record()]);
    let provider = decode(&provider_bytes);
    let provider_identity = provider.identity();
    let mut direct = vec![ConeIdentity::CORE, provider_identity];
    direct.sort_unstable();

    let closure = DecodedCrossConeClosure::new(
        cone_named("current").identity(),
        target(),
        direct,
        vec![decode(&core_bytes), provider],
    )
    .validate_profile_graph()
    .unwrap()
    .validate_identities()
    .unwrap()
    .validate_foundation_structure()
    .unwrap();

    assert_eq!(closure.dependency_first().count(), 2);
    assert_eq!(closure.dependency_count(provider_identity), Some(1));
    assert_eq!(
        closure
            .artifact(provider_identity)
            .unwrap()
            .declared_identity_count(),
        17
    );
}

#[test]
fn closure_foundation_failure_is_attributed_to_the_exact_artifact() {
    let core_bytes = artifact(core_cone(), Vec::new());
    let core = decode(&core_bytes);
    let invalid_cone = cone_named("invalid-foundation");
    let mut invalid_foundation = base_hir_foundation();
    invalid_foundation
        .set_functions(vec![function_record(
            invalid_cone.identity(),
            "missingOrigin",
        )])
        .unwrap();
    let invalid_bytes = artifact_with_foundation(
        invalid_cone,
        vec![core.dependency_record()],
        &invalid_foundation,
    );
    let invalid = decode(&invalid_bytes);
    let invalid_identity = invalid.identity();
    let mut direct = vec![ConeIdentity::CORE, invalid_identity];
    direct.sort_unstable();

    assert!(matches!(
        DecodedCrossConeClosure::new(
            cone_named("current").identity(),
            target(),
            direct,
            vec![decode(&core_bytes), invalid],
        )
        .validate_profile_graph()
        .unwrap()
        .validate_identities()
        .unwrap()
        .validate_foundation_structure(),
        Err(CrossConeClosureFoundationError::Artifact { identity, .. })
            if identity == invalid_identity
    ));
}

#[test]
fn closure_resolves_every_general_hir_interface_after_foundations() {
    let core_bytes = artifact(core_cone(), Vec::new());
    let core = decode(&core_bytes);
    let provider_bytes = artifact(cone_named("provider"), vec![core.dependency_record()]);
    let provider = decode(&provider_bytes);
    let provider_identity = provider.identity();
    let mut direct = vec![ConeIdentity::CORE, provider_identity];
    direct.sort_unstable();

    let closure = DecodedCrossConeClosure::new(
        cone_named("current").identity(),
        target(),
        direct,
        vec![decode(&core_bytes), provider],
    )
    .validate_profile_graph()
    .unwrap()
    .validate_identities()
    .unwrap()
    .validate_foundation_structure()
    .unwrap()
    .resolve_hir_interfaces()
    .unwrap();

    assert_eq!(closure.dependency_first().count(), 2);
    assert!(
        closure
            .artifact(provider_identity)
            .unwrap()
            .hir_interface()
            .public_bindings()
            .records()
            .is_empty()
    );
}

#[test]
fn hir_resolution_failure_is_attributed_to_the_exact_artifact() {
    let core_bytes = artifact(core_cone(), Vec::new());
    let core = decode(&core_bytes);
    let provider_cone = cone_named("unresolved-interface");
    let missing_function = function_record(provider_cone.identity(), "missing");
    let missing_binding =
        CborIdentityRecord::<PersistentExportBindingId, _>::from_key(ExportBindingKey::new(
            provider_cone.identity(),
            PackagePath::root(),
            CanonicalIdentifier::new("missing").unwrap(),
            BindingTarget::function(missing_function.key()).unwrap(),
        ))
        .unwrap();
    let public = PublicExportBindingRecordV1::new(
        missing_binding.id(),
        ExportBindingSourceV1::DeclaredCurrent {
            declaration: BindableEntity::Function(missing_function.id()),
        },
    );
    let provider_bytes = cross_cone_artifact_for(
        provider_cone,
        vec![core.dependency_record()],
        encoded_interface_with_bindings(vec![public]),
    );
    let provider = decode(&provider_bytes);
    let provider_identity = provider.identity();
    let mut direct = vec![ConeIdentity::CORE, provider_identity];
    direct.sort_unstable();

    assert!(matches!(
        DecodedCrossConeClosure::new(
            cone_named("current").identity(),
            target(),
            direct,
            vec![decode(&core_bytes), provider],
        )
        .validate_profile_graph()
        .unwrap()
        .validate_identities()
        .unwrap()
        .validate_foundation_structure()
        .unwrap()
        .resolve_hir_interfaces(),
        Err(CrossConeClosureHirResolutionError::Artifact { identity, .. })
            if identity == provider_identity
    ));
}

fn artifact(cone: ConeRecord, dependencies: Vec<crate::DependencyRecord>) -> Vec<u8> {
    cross_cone_artifact_for(cone, dependencies, empty_cross_cone_hir_interface())
}

fn artifact_with_foundation(
    cone: ConeRecord,
    dependencies: Vec<crate::DependencyRecord>,
    foundation: &CanonicalHirFoundation,
) -> Vec<u8> {
    cross_cone_artifact_for_with_hir_foundation(
        cone,
        dependencies,
        foundation,
        empty_cross_cone_hir_interface(),
    )
}

fn base_hir_foundation() -> CanonicalHirFoundation {
    let mut foundation = CanonicalHirFoundation::empty();
    foundation
        .set_types(vec![
            CoreBuiltinNominal::Unit.identity_record(),
            CoreBuiltinNominal::Any.identity_record(),
        ])
        .unwrap();
    foundation
}

fn function_record(
    origin: ConeIdentity,
    name: &str,
) -> CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> {
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            origin,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    );
    CborIdentityRecord::from_key(declaration).unwrap()
}

fn encoded_interface_with_bindings(bindings: Vec<PublicExportBindingRecordV1>) -> Vec<u8> {
    let mut section = CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(bindings).unwrap(),
        CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
    );
    encode(&section.index_for_wire().unwrap()).unwrap()
}

fn decode(bytes: &[u8]) -> DecodedCrossConeHirFrontSections<'_> {
    open_graph(bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap()
}

fn core_cone() -> ConeRecord {
    ConeRecord::new(
        ConeCoordinate::reserved_core(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap()
}

fn target() -> ValidatedLirTargetSelection {
    ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
}
