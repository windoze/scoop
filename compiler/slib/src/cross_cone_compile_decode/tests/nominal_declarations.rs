use super::nominal_fields::front;
use super::*;
use scoop_hir::{
    CanonicalNestedMemberRefsV1, CanonicalNestedNominalRefsV1, DeclaredVisibilityV1,
    NominalDeclarationDetailsV1, NominalInheritanceModalityV1,
};
use scoop_wire::DecodeLimits;

mod support;
use support::*;

#[test]
fn source_support_queries_never_expose_a_dependency_private_nominal() {
    use crate::cross_cone_hir_authority::CanonicalCrossConeHirSurfaceAuthority;
    use scoop_hir::NominalInterfaceShapeAuthority as _;

    let fixture = Fixture::new();
    let bytes = fixture.artifact(true, true, false);
    let provider = front(&bytes).validate_nominal_surface(vec![]).unwrap();
    let mut identities = fixture.identities();
    let empty: scoop_hir::DecodedCrossConeHirInterfaceSectionV1 =
        scoop_wire::decode_canonical(&empty_cross_cone_hir_interface(), DecodeLimits::default())
            .unwrap();
    let empty = empty.resolve(&mut identities).unwrap();
    let empty_foundation =
        scoop_hir::OdrFreeHirFoundation::try_new(CanonicalHirFoundation::empty()).unwrap();
    let SourceNominalId::Concrete(hidden) = fixture.hidden.declaration() else {
        panic!("concrete private declaration")
    };
    for (local, source_scope) in [(true, false), (true, true), (false, false), (false, true)] {
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
                &empty_foundation,
                &empty,
                vec![provider.nominal_provider_view()],
            )
        };
        let mut meter = scoop_wire::BudgetMeter::new(DecodeLimits::default());
        let authority = CanonicalCrossConeHirSurfaceAuthority::new(
            current,
            &identities,
            foundation,
            interface,
            dependencies,
            &mut meter,
        );
        let mut authority = if source_scope {
            authority.for_source_declarations()
        } else {
            authority
        };
        let result = authority.concrete_nominal_shape(hidden);
        if local && source_scope {
            assert_eq!(result.unwrap().kind(), PublicNominalKindV1::Class);
        } else {
            assert!(
                matches!(result, Err(CrossConeHirNominalAuthorityError::MissingNominalInterface { origin, declaration })
                if origin == provider.identity() && declaration == fixture.hidden.declaration())
            );
        }
    }
}

#[test]
fn ordinary_reader_preserves_private_support_from_the_same_nominal_table() {
    let fixture = Fixture::new();
    let bytes = fixture.artifact(true, true, false);
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
fn ordinary_reader_rejects_missing_support_and_unrelated_support() {
    let fixture = Fixture::new();
    for (include, extra, expected) in [
        (
            false,
            false,
            "required source support declaration is absent",
        ),
        (true, true, "unrelated source support declaration"),
    ] {
        let bytes = fixture.artifact(include, true, extra);
        let Err(CrossConeHirNominalSurfaceError::Declarations(error)) =
            front(&bytes).validate_nominal_surface(vec![])
        else {
            panic!("support closure must reject missing or unrelated declarations")
        };
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[test]
fn ordinary_reader_rejects_a_private_child_omitted_from_its_declaring_parent() {
    let fixture = Fixture::new();
    let bytes = fixture.artifact(false, false, false);
    let Err(CrossConeHirNominalSurfaceError::Relations(
        scoop_hir::NominalDeclarationInventoryError::MissingChild { owner, child },
    )) = front(&bytes).validate_nominal_surface(vec![])
    else {
        panic!("all nominal children in the foundation must remain declared")
    };
    assert_eq!(owner, fixture.public.declaration());
    assert_eq!(child, fixture.hidden.declaration());
}
