use super::nominal_fields::front;
use super::*;
use scoop_hir::{
    CanonicalNestedMemberRefsV1, CanonicalNestedNominalRefsV1, DeclaredVisibilityV1,
    NominalDeclarationDetailsV1, NominalInheritanceModalityV1,
};

mod support;
use support::*;

#[test]
fn signature_queries_resolve_actual_local_and_dependency_declarations() {
    use crate::cross_cone_hir_authority::CanonicalCrossConeHirSurfaceAuthority;
    use scoop_hir::NominalInterfaceShapeAuthority as _;

    let fixture = Fixture::new();
    let bytes = fixture.artifact(true, true);
    let provider = front(&bytes).validate_nominal_surface(vec![]).unwrap();
    let mut identities = fixture.identities();
    let empty: scoop_hir::DecodedCrossConeHirInterfaceSectionV1 =
        scoop_wire::decode_canonical(&empty_cross_cone_hir_interface()).unwrap();
    let empty = empty.resolve(&mut identities).unwrap();
    let empty_foundation =
        scoop_hir::OdrFreeHirFoundation::try_new(CanonicalHirFoundation::empty()).unwrap();
    let SourceNominalId::Concrete(hidden) = fixture.hidden.declaration() else {
        panic!("concrete private declaration")
    };
    for local in [true, false] {
        let (current, foundation, interface, dependencies) = if local {
            (
                provider.identity(),
                provider.hir_foundation(),
                provider.hir_interface(),
                vec![],
            )
        } else {
            (
                scoop_identity::ConeIdentity::SINGLE_FILE,
                empty_foundation.as_canonical(),
                &empty,
                vec![provider.nominal_provider_view()],
            )
        };

        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            current,
            &identities,
            foundation,
            interface,
            dependencies,
        );
        let result = authority.concrete_nominal_shape(hidden);
        assert_eq!(result.unwrap().kind(), PublicNominalKindV1::Class);
    }
}

#[test]
fn ordinary_reader_preserves_private_support_from_the_same_nominal_table() {
    let fixture = Fixture::new();
    let bytes = fixture.artifact(true, true);
    let validated = front(&bytes).validate_nominal_surface(vec![]).unwrap();
    let table = validated.hir_interface().nominal_interfaces();
    assert_eq!(table.records().len(), 1);
    assert_eq!(table.support_records().len(), 1);
    let hidden = fixture.hidden.declaration();
    assert!(table.get(hidden).is_none());
    assert_eq!(
        table
            .declaration(hidden)
            .unwrap()
            .declaration_details()
            .declared_visibility(),
        DeclaredVisibilityV1::Private
    );
}

#[test]
fn ordinary_reader_rejects_a_declared_child_missing_from_the_shared_table() {
    let fixture = Fixture::new();
    let bytes = fixture.artifact(false, true);
    let Err(CrossConeHirSourceInterfaceSurfaceError::SourceInventory(error)) = front(&bytes)
        .validate_nominal_surface(vec![])
        .unwrap()
        .validate_property_surface(vec![])
        .unwrap()
        .validate_callable_surface(vec![])
        .unwrap()
        .validate_type_alias_surface(vec![])
        .unwrap()
        .validate_source_interfaces(vec![])
    else {
        panic!("declared children must have complete source records")
    };
    assert!(
        error
            .to_string()
            .contains("required shared source declaration is absent"),
        "{error}"
    );
}

#[test]
fn ordinary_reader_rejects_a_private_child_omitted_from_its_declaring_parent() {
    let fixture = Fixture::new();
    let bytes = fixture.artifact(false, false);
    let Err(CrossConeHirNominalSurfaceError::Relations(
        scoop_hir::NominalDeclarationInventoryError::MissingChild { owner, child },
    )) = front(&bytes).validate_nominal_surface(vec![])
    else {
        panic!("all nominal children in the foundation must remain declared")
    };
    assert_eq!(owner, fixture.public.declaration());
    assert_eq!(child, fixture.hidden.declaration());
}
