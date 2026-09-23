use super::*;
use crate::CrossConeHirDefaultTypeAccessError as Error;
use scoop_hir::{DeclaredVisibilityV1, OdrFreeHirFoundation};
use scoop_identity::{Effect, NonEmptyVec};
use scoop_wire::WirePath;

mod providers;
mod support;

#[test]
fn ordinary_reader_replays_private_and_internal_nominal_target_domains() {
    for visibility in [
        DeclaredVisibilityV1::Private,
        DeclaredVisibilityV1::Internal,
    ] {
        let (mut fixture, ty) = support::hidden(visibility, false, false);
        support::add_unused_local(&mut fixture, ty);
        assert!(matches!(failure(&fixture), Error::WitnessDomain));
    }
}

#[test]
fn ordinary_reader_intersects_the_complete_lexical_owner_chain() {
    let (mut fixture, ty) = support::hidden(DeclaredVisibilityV1::Private, false, true);
    support::add_unused_local(&mut fixture, ty);
    assert!(matches!(failure(&fixture), Error::WitnessDomain));
}

#[test]
fn ordinary_reader_checks_source_only_generic_type_access() {
    let (mut fixture, ty) = support::hidden(DeclaredVisibilityV1::Internal, true, false);
    support::add_unused_local(&mut fixture, ty);
    assert!(matches!(failure(&fixture), Error::WitnessDomain));
}

#[test]
fn ordinary_reader_intersects_generic_tuple_and_function_constituents() {
    for case in 0..4 {
        let (mut fixture, hidden) = support::hidden(DeclaredVisibilityV1::Private, false, false);
        let public = fixture.interface.default_templates().records()[0]
            .result()
            .clone();
        let ty = match case {
            0 => SignatureTypeKey::Tuple(NonEmptyVec::from_first(public, [hidden])),
            1 => SignatureTypeKey::Function {
                effect: Effect::Ordinary,
                parameters: vec![hidden],
                result: Box::new(public),
            },
            2 => SignatureTypeKey::Function {
                effect: Effect::Ordinary,
                parameters: vec![public],
                result: Box::new(hidden),
            },
            3 => {
                let origin = fixture
                    .interface
                    .nominal_interfaces()
                    .records()
                    .iter()
                    .find_map(|record| match record.declaration() {
                        SourceNominalId::GenericTemplate(id) => Some(id),
                        _ => None,
                    })
                    .unwrap();
                SignatureTypeKey::NominalApplication {
                    origin,
                    arguments: NonEmptyVec::from_first(hidden, []),
                }
            }
            _ => unreachable!(),
        };
        support::add_unused_local(&mut fixture, ty);
        assert!(
            matches!(failure(&fixture), Error::WitnessDomain),
            "case {case}"
        );
    }
}

#[test]
fn ordinary_reader_requires_actual_protocol_roles_for_pointer_wrappers() {
    for function in [false, true] {
        let mut fixture = default_fixture::fixture(default_fixture::Case::Defined);
        let public = fixture.interface.default_templates().records()[0]
            .result()
            .clone();
        let ty = if function {
            SignatureTypeKey::NativeFunctionPointer {
                calling_convention: scoop_identity::CallingConvention::C,
                parameters: vec![public.clone()],
                result: Box::new(public),
            }
        } else {
            SignatureTypeKey::RawPointer(Box::new(public))
        };
        support::add_unused_local(&mut fixture, ty);
        assert!(matches!(failure(&fixture), Error::MissingPointerProtocol));
    }
}

#[test]
fn ordinary_reader_type_access_uses_the_existing_artifact_budget() {
    let fixture = default_fixture::fixture(default_fixture::Case::Defined);
    let bytes = fixture.artifact();
    let mut front = support::front(&bytes);
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
        .charge_work(remaining - work + 1, &WirePath::root())
        .unwrap();
    let Err(Error::Reference { source, .. }) = support::validate(&mut front) else {
        panic!("type domain replay must not reset the artifact budget")
    };
    assert!(matches!(*source, Error::Resource(_)));
    assert!(format!("{source:?}").contains("ValidationWorkUnits"));
}

fn failure(fixture: &CallableSourceSurface) -> Error {
    let bytes = fixture.artifact();
    let result = validate_until_type_alias(&bytes).validate_source_interfaces(vec![]);
    let Err(CrossConeHirSourceInterfaceSurfaceError::DefaultTypeAccess(Error::Reference {
        template,
        source,
        ..
    })) = result
    else {
        panic!(
            "default type target must be checked against source metadata: {:?}",
            result.err()
        )
    };
    assert_eq!(template.owner(), fixture.owner);
    *source
}
