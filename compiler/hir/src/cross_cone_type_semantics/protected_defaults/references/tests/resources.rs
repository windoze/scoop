use scoop_identity::{OptionalSignatureType, SignatureTypeKey};
use scoop_wire::{BudgetMeter, DecodeLimits};

use super::support::*;
use crate::{
    DefaultBinderRefV1, DefaultBoundCallableRefV1, DefaultBoundCallableSourceV1,
    DefaultCallableDeclarationV1, DefaultCallableRefV1, DefaultConstructorRefV1, DefaultFieldRefV1,
    ExportDefaultCallableTargetV1,
};

#[test]
fn aggregate_resolution_obeys_shared_heap_work_nodes_and_origin_bytes() {
    let f = Fixture::new();
    let set = full_set(&f);
    for limits in [
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            decoded_nodes: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            decoded(&set).resolve(&mut f.resolver(), &mut BudgetMeter::new(limits)),
            Err(ProtectedDefaultReferenceSetResolutionError::Resource(_))
        ));
    }
    assert!(matches!(
        decoded(&set).resolve(
            &mut f.resolver(),
            &mut BudgetMeter::new(DecodeLimits {
                owned_bytes: 0,
                ..DecodeLimits::default()
            })
        ),
        Err(ProtectedDefaultReferenceSetResolutionError::Record {
            error: ProtectedDefaultReferenceResolutionError::Resource(_),
            ..
        })
    ));
    let mut initial = meter();
    decoded(&set)
        .resolve(&mut f.resolver(), &mut initial)
        .unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        decoded_nodes: initial.usage().decoded_nodes,
        ..DecodeLimits::default()
    });
    decoded(&set)
        .resolve(&mut f.resolver(), &mut shared)
        .unwrap();
    assert!(matches!(
        decoded(&set).resolve(&mut f.resolver(), &mut shared),
        Err(ProtectedDefaultReferenceSetResolutionError::Resource(_))
    ));
}

#[test]
fn every_signature_bearing_target_charges_nested_resolution() {
    let f = Fixture::new();
    let nested = SignatureTypeKey::RawPointer(Box::new(SignatureTypeKey::Nominal(f.type_id)));
    let owner_callable = DefaultCallableRefV1::try_new(
        DefaultCallableDeclarationV1::Function(f.function),
        OptionalSignatureType::Present(Box::new(nested.clone())),
        vec![],
    )
    .unwrap();
    let argument_callable = DefaultCallableRefV1::try_new(
        DefaultCallableDeclarationV1::Function(f.function),
        OptionalSignatureType::Absent,
        vec![nested.clone()],
    )
    .unwrap();
    let mut sets = Vec::new();
    for target in [
        ExportDefaultCallableTargetV1::Callable(owner_callable),
        ExportDefaultCallableTargetV1::Callable(argument_callable),
        ExportDefaultCallableTargetV1::DerivedEquality {
            owner_type: nested.clone(),
        },
        ExportDefaultCallableTargetV1::Bound(DefaultBoundCallableRefV1::new(
            DefaultBinderRefV1::new(0, 0),
            DefaultBoundCallableSourceV1::Class {
                bound: nested.clone(),
                callable: f.callable(),
            },
            f.value_type(),
        )),
        ExportDefaultCallableTargetV1::Bound(DefaultBoundCallableRefV1::new(
            DefaultBinderRefV1::new(0, 0),
            DefaultBoundCallableSourceV1::Interface {
                bound: nested.clone(),
                member: scoop_identity::CallableTemplateOrigin::Function(f.function),
            },
            f.value_type(),
        )),
        ExportDefaultCallableTargetV1::Bound(DefaultBoundCallableRefV1::new(
            DefaultBinderRefV1::new(0, 0),
            DefaultBoundCallableSourceV1::Class {
                bound: f.value_type(),
                callable: f.callable(),
            },
            nested.clone(),
        )),
    ] {
        let mut set = empty_set();
        set.callables = vec![record(&f, target)];
        sets.push(set);
    }
    let mut set = empty_set();
    set.types = vec![record(&f, nested.clone())];
    sets.push(set);
    let mut set = empty_set();
    set.constructors = vec![record(
        &f,
        DefaultConstructorRefV1::Struct {
            declaration: f.constructor,
            owner_type: nested.clone(),
        },
    )];
    sets.push(set);
    let mut set = empty_set();
    set.fields = vec![record(
        &f,
        DefaultFieldRefV1::Class {
            declaration: f.field,
            owner_type: nested,
        },
    )];
    sets.push(set);
    for set in sets {
        for limits in [
            DecodeLimits {
                semantic_recursion: 1,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_edges: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(matches!(
                decoded(&set).resolve(&mut f.resolver(), &mut BudgetMeter::new(limits)),
                Err(ProtectedDefaultReferenceSetResolutionError::Record {
                    error: ProtectedDefaultReferenceResolutionError::Resource(_),
                    ..
                })
            ));
        }
        assert_eq!(
            decoded(&set)
                .resolve(&mut f.resolver(), &mut meter())
                .unwrap(),
            set
        );
    }
}
