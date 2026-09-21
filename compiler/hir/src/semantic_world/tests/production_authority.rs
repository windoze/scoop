use scoop_identity::{BindingTarget, ConeIdentity, SemanticIdentitySession};

use super::super::*;
use super::fixture::{
    ProviderFixture, certificate, coordinate, empty_alias_expansions, import_foundation, package,
};
use crate::{
    CanonicalHirFoundation, CanonicalPublicExportBindingsV1, ExternalHirReferenceSemanticAuthority,
    ExternalHirTargetV1, PublicExportBindingClosureAuthority,
};

#[test]
fn production_authority_resolves_current_and_imported_targets_by_typed_identity() {
    let current_fixture = ProviderFixture::with_nominals(
        coordinate("current"),
        package(&["consumer"]),
        "CurrentType",
        None,
    );
    let core = ProviderFixture::empty(scoop_identity::ConeCoordinate::reserved_core());
    let direct = ProviderFixture::with_nominals(
        coordinate("direct"),
        package(&["dependency"]),
        "ImportedType",
        None,
    );
    let support = ProviderFixture::with_nominals(
        coordinate("support"),
        package(&["support"]),
        "SupportType",
        None,
    );
    let mut session = SemanticIdentitySession::new();
    let core_foundation = import_foundation(&mut session, &core, 1);
    let direct_foundation = import_foundation(&mut session, &direct, 2);
    let support_foundation = import_foundation(&mut session, &support, 3);
    let aliases = empty_alias_expansions();
    let world = ImportedSemanticWorld::from_validated_closure(
        current_fixture.identity(),
        vec![
            DirectImportedProviderInput::from_validated(
                certificate(&core.coordinate, 1),
                &core_foundation,
                &core.interface,
                &aliases,
            ),
            DirectImportedProviderInput::from_validated(
                certificate(&direct.coordinate, 2),
                &direct_foundation,
                &direct.interface,
                &aliases,
            ),
        ],
        vec![SupportImportedProviderInput::from_validated(
            certificate(&support.coordinate, 3),
            &support_foundation,
            &support.interface,
            &aliases,
        )],
    )
    .unwrap();
    let mut authority = CrossConeHirProductionAuthority::new(
        &current_fixture.foundation,
        current_fixture.interface.public_bindings(),
        &world,
    );

    for fixture in [&current_fixture, &direct, &support] {
        let target = ExternalHirTargetV1::Nominal(fixture.outer.unwrap());
        assert_eq!(
            authority.external_hir_target_origin(target).unwrap(),
            fixture.identity()
        );
        assert_eq!(
            authority.external_hir_target_binding_root(target).unwrap(),
            BindingTarget::type_name(fixture.outer_key.as_ref().unwrap()).unwrap()
        );
    }

    assert!(authority.is_direct_dependency(ConeIdentity::CORE));
    assert!(authority.is_direct_dependency(direct.identity()));
    assert!(!authority.is_direct_dependency(support.identity()));
    assert_eq!(authority.closure_node_count(), 4);
}

#[test]
fn production_authority_exposes_exact_binding_surfaces_without_support_enumeration() {
    let current = coordinate("current").identity().unwrap();
    let core = ProviderFixture::empty(scoop_identity::ConeCoordinate::reserved_core());
    let direct = ProviderFixture::with_nominals(
        coordinate("direct"),
        package(&["dependency"]),
        "ImportedType",
        None,
    );
    let mut session = SemanticIdentitySession::new();
    let core_foundation = import_foundation(&mut session, &core, 1);
    let direct_foundation = import_foundation(&mut session, &direct, 2);
    let aliases = empty_alias_expansions();
    let world = ImportedSemanticWorld::from_validated_closure(
        current,
        vec![
            DirectImportedProviderInput::from_validated(
                certificate(&core.coordinate, 1),
                &core_foundation,
                &core.interface,
                &aliases,
            ),
            DirectImportedProviderInput::from_validated(
                certificate(&direct.coordinate, 2),
                &direct_foundation,
                &direct.interface,
                &aliases,
            ),
        ],
        Vec::new(),
    )
    .unwrap();
    let current_foundation = CanonicalHirFoundation::empty();
    let current_bindings = CanonicalPublicExportBindingsV1::default();
    let authority =
        CrossConeHirProductionAuthority::new(&current_foundation, &current_bindings, &world);
    let binding = direct.outer_binding.unwrap();

    assert_eq!(
        authority.binding_key(binding).unwrap().exporter(),
        direct.identity()
    );
    assert_eq!(
        authority
            .public_bindings(direct.identity())
            .unwrap()
            .records()
            .len(),
        1
    );
    assert!(authority.public_bindings(current).is_some());
    let missing_fixture = ProviderFixture::with_nominals(
        coordinate("absent"),
        package(&["absent"]),
        "MissingType",
        None,
    );
    let missing = ExternalHirTargetV1::Nominal(missing_fixture.outer.unwrap());
    let mut authority = authority;
    assert!(matches!(
        authority.external_hir_target_origin(missing),
        Err(CrossConeHirProductionAuthorityError::MissingCanonicalKey { target })
            if target == missing
    ));
}
