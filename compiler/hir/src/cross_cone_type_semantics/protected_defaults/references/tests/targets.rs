use scoop_identity::{
    CallableTemplateOrigin, CallingConvention, Effect, NonEmptyVec, SignatureTypeKey,
};
use scoop_wire::WirePath;

use super::support::*;
use crate::{
    DefaultBinderRefV1, DefaultBoundCallableRefV1, DefaultBoundCallableSourceV1,
    DefaultCallableDeclarationV1, DefaultCallableReferenceTargetViewV1,
    DefaultClassConstructorIdV1, DefaultConstructorRefV1, DefaultConstructorReferenceTargetViewV1,
    DefaultEnumVariantRefV1, DefaultFieldRefV1, DefaultFieldReferenceTargetViewV1,
    ExportDefaultCallableTargetV1, compare_default_signature_reference_targets,
};

#[test]
fn borrowed_callable_comparisons_preserve_actual_to_declared_direction_and_kind() {
    let f = Fixture::new();
    let targets = vec![
        ExportDefaultCallableTargetV1::Callable(f.callable()),
        ExportDefaultCallableTargetV1::Bound(DefaultBoundCallableRefV1::new(
            DefaultBinderRefV1::new(0, 0),
            DefaultBoundCallableSourceV1::Class {
                bound: f.value_type(),
                callable: f.callable(),
            },
            f.value_type(),
        )),
        ExportDefaultCallableTargetV1::DerivedEquality {
            owner_type: f.value_type(),
        },
        ExportDefaultCallableTargetV1::LocalFunction {
            declaration: CallableTemplateOrigin::Function(f.function),
        },
        ExportDefaultCallableTargetV1::Lambda { body: f.generated },
        ExportDefaultCallableTargetV1::AnonymousFunction { body: f.generated },
        ExportDefaultCallableTargetV1::CallableReference {
            invoke: f.generated,
        },
        ExportDefaultCallableTargetV1::FunctionAddress {
            declaration: DefaultCallableDeclarationV1::Function(f.function),
        },
    ];
    for left in &targets {
        for right in &targets {
            assert_eq!(
                DefaultCallableReferenceTargetViewV1::from(left)
                    .compare_to(right, &WirePath::root())
                    .unwrap(),
                left.cmp(right)
            );
        }
    }
    let mut set = empty_set();
    set.callables = targets
        .into_iter()
        .rev()
        .map(|target| record(&f, target))
        .collect();
    let set = ProtectedDefaultReferenceSetV1::try_new(
        set.callables,
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    assert_eq!(decoded(&set).resolve(&mut f.resolver()).unwrap(), set);
}

#[test]
fn constructor_and_field_views_preserve_projections_and_canonical_order() {
    let f = Fixture::new();
    let constructors = vec![
        DefaultConstructorRefV1::Struct {
            declaration: f.constructor,
            owner_type: f.value_type(),
        },
        DefaultConstructorRefV1::Class {
            declaration: DefaultClassConstructorIdV1::Source(f.constructor),
            owner_type: f.value_type(),
        },
        DefaultConstructorRefV1::Variant {
            declaration: f.variant,
            owner_type: f.value_type(),
        },
    ];
    let fields = vec![
        DefaultFieldRefV1::Struct {
            declaration: f.field,
            owner_type: f.value_type(),
        },
        DefaultFieldRefV1::Tuple {
            declaration_index: 1,
        },
        DefaultFieldRefV1::Tuple {
            declaration_index: 2,
        },
        DefaultFieldRefV1::Class {
            declaration: f.field,
            owner_type: f.value_type(),
        },
    ];
    for left in &constructors {
        for right in &constructors {
            assert_eq!(
                DefaultConstructorReferenceTargetViewV1::from(left)
                    .compare_to(right, &WirePath::root())
                    .unwrap(),
                left.cmp(right)
            );
        }
    }
    for left in &fields {
        for right in &fields {
            assert_eq!(
                DefaultFieldReferenceTargetViewV1::from(left)
                    .compare_to(right, &WirePath::root())
                    .unwrap(),
                left.cmp(right)
            );
        }
    }
    let variant = DefaultEnumVariantRefV1::new(f.variant, f.value_type());
    assert!(
        DefaultConstructorReferenceTargetViewV1::Variant(&variant)
            .compare_to(&constructors[2], &WirePath::root())
            .unwrap()
            .is_eq()
    );
    let owner_type = f.value_type();
    assert!(
        DefaultFieldReferenceTargetViewV1::Struct {
            declaration: f.field,
            owner_type: &owner_type
        }
        .compare_to(&fields[0], &WirePath::root())
        .unwrap()
        .is_eq()
    );
    let set = ProtectedDefaultReferenceSetV1::try_new(
        vec![],
        constructors
            .into_iter()
            .map(|target| record(&f, target))
            .collect(),
        vec![],
        vec![],
        vec![],
        fields
            .into_iter()
            .map(|target| record(&f, target))
            .collect(),
    )
    .unwrap();
    assert_eq!(decoded(&set).resolve(&mut f.resolver()).unwrap(), set);
}

#[test]
fn signature_target_comparison_reuses_full_structural_order() {
    let f = Fixture::new();
    let types = vec![
        SignatureTypeKey::Nominal(f.type_id),
        SignatureTypeKey::Tuple(NonEmptyVec::from_first(f.value_type(), [])),
        SignatureTypeKey::Function {
            effect: Effect::Ordinary,
            parameters: vec![],
            result: Box::new(f.value_type()),
        },
        SignatureTypeKey::RawPointer(Box::new(f.value_type())),
        SignatureTypeKey::NativeFunctionPointer {
            calling_convention: CallingConvention::C,
            parameters: vec![f.value_type()],
            result: Box::new(f.value_type()),
        },
        f.value_type(),
        SignatureTypeKey::Binder { depth: 0, index: 1 },
    ];
    for left in &types {
        for right in &types {
            assert_eq!(
                compare_default_signature_reference_targets(left, right, &WirePath::root())
                    .unwrap(),
                left.cmp(right)
            );
        }
    }
    let mut set = empty_set();
    set.types = types
        .into_iter()
        .rev()
        .map(|target| record(&f, target))
        .collect();
    let set =
        ProtectedDefaultReferenceSetV1::try_new(vec![], vec![], set.types, vec![], vec![], vec![])
            .unwrap();
    assert_eq!(decoded(&set).resolve(&mut f.resolver()).unwrap(), set);
}

#[test]
fn local_function_target_rejects_constructor_owner_in_producer_and_reader() {
    let f = Fixture::new();
    let mut set = empty_set();
    set.callables = vec![record(
        &f,
        ExportDefaultCallableTargetV1::LocalFunction {
            declaration: CallableTemplateOrigin::Constructor(f.constructor),
        },
    )];
    assert!(matches!(
        decoded(&set).resolve(&mut f.resolver()),
        Err(ProtectedDefaultReferenceSetResolutionError::Record {
            error: ProtectedDefaultReferenceResolutionError::Target(_),
            ..
        })
    ));
    assert!(matches!(
        ProtectedDefaultReferenceSetV1::try_new(
            set.callables,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![]
        ),
        Err(ProtectedDefaultReferenceSetBuildError::CallableTarget { index: 0, .. })
    ));
}
