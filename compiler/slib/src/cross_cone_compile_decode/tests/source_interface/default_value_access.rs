use super::super::surface_fixture::declaration_front;
use super::*;
use crate::cross_cone_hir_authority::{
    CanonicalCrossConeHirSurfaceAuthority, CrossConeHirDefaultValueAccessError as Error,
};
use scoop_hir::DefaultSourceValueTargetV1 as Target;
use scoop_hir::*;
use scoop_wire::WirePath;

mod declarations;
mod providers;
mod support;

#[test]
fn value_access_replays_shared_constructor_and_global_visibility() {
    let (bytes, targets) = support::surface(cone());
    let DefaultConstructorRefV1::Struct { declaration, .. } = targets.constructor else {
        panic!("struct fixture")
    };
    for visibility in [
        DeclaredVisibilityV1::Private,
        DeclaredVisibilityV1::Internal,
    ] {
        for constructor in [true, false] {
            let mut front = declaration_front(&bytes);
            let (target, subject, kind) = if constructor {
                (
                    Target::Constructor(&targets.constructor),
                    DefinitionOriginSubject::Constructor(declaration),
                    ExportDefaultReferenceKindV1::Constructor,
                )
            } else {
                (
                    Target::Global(targets.global),
                    DefinitionOriginSubject::Property(targets.global),
                    ExportDefaultReferenceKindV1::Global,
                )
            };
            support::insert_reference(&mut front, target);
            support::validate(&mut front).unwrap();
            declarations::restrict(&mut front, subject, visibility);
            assert!(matches!(
                support::failure(&mut front, kind),
                Error::WitnessDomain
            ));
        }
    }
}

#[test]
fn value_access_intersects_constructor_and_field_owner_visibility() {
    let (bytes, targets) = support::surface(cone());
    for (target, kind) in [
        (
            Target::Constructor(&targets.constructor),
            ExportDefaultReferenceKindV1::Constructor,
        ),
        (
            Target::Field(&targets.field),
            ExportDefaultReferenceKindV1::Field,
        ),
    ] {
        let mut front = declaration_front(&bytes);
        support::insert_reference(&mut front, target);
        support::validate(&mut front).unwrap();
        declarations::restrict(
            &mut front,
            DefinitionOriginSubject::Type(targets.nominal),
            DeclaredVisibilityV1::Private,
        );
        assert!(matches!(
            support::failure(&mut front, kind),
            Error::WitnessDomain
        ));
    }
}

#[test]
fn value_access_rejects_wrong_applied_owners_and_field_roles() {
    let (bytes, targets) = support::surface(cone());
    let DefaultFieldRefV1::Struct { declaration, .. } = targets.field else {
        panic!("field fixture")
    };
    let wrong_owner = DefaultFieldRefV1::Struct {
        declaration,
        owner_type: SignatureTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Any
                .identity_record()
                .id(),
        ),
    };
    let wrong_role = DefaultFieldRefV1::Class {
        declaration,
        owner_type: SignatureTypeKey::Nominal(targets.nominal),
    };
    for (target, role) in [(&wrong_owner, false), (&wrong_role, true)] {
        let mut front = declaration_front(&bytes);
        support::insert_reference(&mut front, Target::Field(target));
        let Error::Target(error) =
            support::failure(&mut front, ExportDefaultReferenceKindV1::Field)
        else {
            panic!("identity route error")
        };
        if role {
            assert!(matches!(*error, DefaultSourceTargetSubjectError::Role(_)));
        } else {
            assert!(matches!(
                *error,
                DefaultSourceTargetSubjectError::AppliedOwner(_)
            ));
        }
    }
}

#[test]
fn tuple_value_access_is_structural_and_keeps_the_existing_budget() {
    let (bytes, _) = support::surface(cone());
    let mut front = declaration_front(&bytes);
    support::insert_reference(
        &mut front,
        Target::Field(&DefaultFieldRefV1::Tuple {
            declaration_index: 0,
        }),
    );
    let before = front
        .graph
        .envelope
        .meter_mut()
        .usage()
        .validation_work_units;
    support::validate(&mut front).unwrap();
    let meter = front.graph.envelope.meter_mut();
    let work = meter.usage().validation_work_units - before;
    assert!(work > 1);
    let remaining = meter.limits().validation_work_units - meter.usage().validation_work_units;
    meter
        .charge_work(remaining - (work - 1), &WirePath::root())
        .unwrap();
    assert!(matches!(
        support::failure(&mut front, ExportDefaultReferenceKindV1::Field),
        Error::Resource(_)
    ));
}
