use super::*;
use scoop_hir::{
    CallableSourceInterfaceSemanticAuthority, CanonicalConstValueKindV1,
    ExportConstValueSemanticAuthority, IntrinsicTypeKind,
};
use scoop_identity::PendingIdentityValidation;

use crate::cross_cone_hir_authority::{
    CanonicalCrossConeHirSurfaceAuthority, CrossConeHirCallableSourceAuthorityError,
};

mod support;
use support::*;

#[test]
fn intrinsic_queries_follow_all_actual_typed_references_across_reachable_providers() {
    with_base(|base| {
        let mut pending = PendingIdentityValidation::new();
        let families = std::iter::once(IntrinsicTypeKind::Boolean)
            .chain(
                scoop_hir::IntegerKind::ALL
                    .into_iter()
                    .map(IntrinsicTypeKind::Integer),
            )
            .chain([
                IntrinsicTypeKind::Char,
                IntrinsicTypeKind::String,
                IntrinsicTypeKind::Array,
                IntrinsicTypeKind::MutableArray,
                IntrinsicTypeKind::Ptr,
                IntrinsicTypeKind::FunPtr,
            ])
            .chain(
                scoop_hir::AtomicValueKind::ALL
                    .iter()
                    .copied()
                    .map(IntrinsicTypeKind::Atomic),
            );
        let providers = families
            .enumerate()
            .map(|(index, family)| {
                let identity = if index == 0 {
                    base.identity()
                } else {
                    provider_identity(index)
                };
                Provider::new(identity, family, &mut pending)
            })
            .collect::<Vec<_>>();
        let identities = pending.finish().unwrap();

        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            base.identity(),
            &identities,
            base.hir_foundation(),
            &providers[0].interface,
            providers[1..]
                .iter()
                .map(|provider| provider.view(base))
                .collect(),
        );
        for provider in &providers {
            match provider.family {
                IntrinsicTypeKind::Atomic(
                    scoop_hir::AtomicValueKind::Int
                    | scoop_hir::AtomicValueKind::Long
                    | scoop_hir::AtomicValueKind::Boolean,
                ) => assert!(
                    authority
                        .validate_const_value_type(
                            concrete(provider.owner),
                            CanonicalConstValueKindV1::Boolean
                        )
                        .is_err()
                ),
                IntrinsicTypeKind::Unit | IntrinsicTypeKind::Any | IntrinsicTypeKind::Nothing => {
                    assert!(
                        authority
                            .validate_const_value_type(
                                concrete(provider.owner),
                                CanonicalConstValueKindV1::Boolean,
                            )
                            .is_err()
                    )
                }
                IntrinsicTypeKind::Integer(kind) => authority
                    .validate_const_value_type(
                        concrete(provider.owner),
                        CanonicalConstValueKindV1::Integer(kind),
                    )
                    .unwrap(),
                IntrinsicTypeKind::Float(kind) => authority
                    .validate_const_value_type(
                        concrete(provider.owner),
                        CanonicalConstValueKindV1::Float(kind),
                    )
                    .unwrap(),
                IntrinsicTypeKind::Char => authority
                    .validate_const_value_type(
                        concrete(provider.owner),
                        CanonicalConstValueKindV1::Char,
                    )
                    .unwrap(),
                IntrinsicTypeKind::Boolean => authority
                    .validate_const_value_type(
                        concrete(provider.owner),
                        CanonicalConstValueKindV1::Boolean,
                    )
                    .unwrap(),
                IntrinsicTypeKind::String => authority
                    .validate_const_value_type(
                        concrete(provider.owner),
                        CanonicalConstValueKindV1::String,
                    )
                    .unwrap(),
                IntrinsicTypeKind::Array => authority
                    .validate_array_type(generic(provider.owner))
                    .unwrap(),
                actual @ (IntrinsicTypeKind::MutableArray
                | IntrinsicTypeKind::MaybeUninit
                | IntrinsicTypeKind::Atomic(scoop_hir::AtomicValueKind::Reference)
                | IntrinsicTypeKind::Ptr
                | IntrinsicTypeKind::FunPtr) => {
                    let Err(CrossConeHirCallableSourceAuthorityError::ArrayType(error)) =
                        authority.validate_array_type(generic(provider.owner))
                    else {
                        panic!("only the declared Array family satisfies a vararg type");
                    };
                    assert!(matches!(*error, CrossConeHirIntrinsicTypeError::Family {
                        declaration, expected: IntrinsicTypeKind::Array, actual: found,
                    } if declaration == provider.owner && found == actual));
                }
            }
        }
        // A matching family exists on another provider; the actual typed id must win.
        let byte = &providers[1];
        assert!(
            matches!(authority.validate_const_value_type(concrete(byte.owner),
            CanonicalConstValueKindV1::Integer(scoop_hir::IntegerKind::SIGNED_32)),
            Err(CrossConeHirConstAuthorityError::ValueType(CrossConeHirIntrinsicTypeError::Family {
                declaration, actual: IntrinsicTypeKind::Integer(scoop_hir::IntegerKind::SIGNED_8), ..
            })) if declaration == byte.owner)
        );
    });
}

#[test]
fn intrinsic_queries_reject_unreachable_missing_and_non_intrinsic_source_records() {
    with_base(|base| {
        let mut pending = PendingIdentityValidation::new();
        let local = Provider::new(base.identity(), IntrinsicTypeKind::Boolean, &mut pending);
        let mut dependency =
            Provider::new(provider_identity(1), IntrinsicTypeKind::Array, &mut pending);
        let identities = pending.finish().unwrap();

        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            base.identity(),
            &identities,
            base.hir_foundation(),
            &local.interface,
            vec![],
        );
        let Err(CrossConeHirCallableSourceAuthorityError::ArrayType(error)) =
            authority.validate_array_type(generic(dependency.owner))
        else {
            panic!("registered identities do not grant dependency reachability");
        };
        assert!(matches!(*error, CrossConeHirIntrinsicTypeError::Nominal(
            CrossConeHirNominalAuthorityError::UnreachableProvider { origin }) if origin == dependency.identity));

        dependency.replace_shape(NominalSourceShapeV1::Class(Default::default()));
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            base.identity(),
            &identities,
            base.hir_foundation(),
            &local.interface,
            vec![dependency.view(base)],
        );
        let Err(CrossConeHirCallableSourceAuthorityError::ArrayType(error)) =
            authority.validate_array_type(generic(dependency.owner))
        else {
            panic!("an ordinary class is not an intrinsic Array");
        };
        assert!(
            matches!(*error, CrossConeHirIntrinsicTypeError::NotIntrinsic { declaration, .. }
            if declaration == dependency.owner)
        );

        dependency.interface = CrossConeHirInterfaceSectionV1::empty();
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            base.identity(),
            &identities,
            base.hir_foundation(),
            &local.interface,
            vec![dependency.view(base)],
        );
        let Err(CrossConeHirCallableSourceAuthorityError::ArrayType(error)) =
            authority.validate_array_type(generic(dependency.owner))
        else {
            panic!("an identity without its source interface cannot supply a type");
        };
        assert!(matches!(*error, CrossConeHirIntrinsicTypeError::Nominal(
            CrossConeHirNominalAuthorityError::MissingNominalInterface { declaration, .. })
            if declaration == dependency.owner));
    });
}
