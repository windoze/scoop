use scoop_identity::{
    CallingConvention, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionOwnerChain, Effect, NonEmptyVec, PackagePath, PersistentGenericTypeId,
    SignatureTypeKey, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

use super::*;
use crate::{CanonicalSignatureTypesV1, NominalTypeParameterBoundsV1, TypeParameterBinderV1};

#[test]
fn declaration_scope_places_only_nonempty_frames() {
    let own_and_owner = SignatureBinderScopeV1::for_declaration(2, Some(3));
    assert_eq!(own_and_owner.available_depths(), 2);
    assert_eq!(own_and_owner.arity_at_depth(0), Some(2));
    assert_eq!(own_and_owner.arity_at_depth(1), Some(3));

    let owner_only = SignatureBinderScopeV1::for_declaration(0, Some(3));
    assert_eq!(owner_only.available_depths(), 1);
    assert_eq!(owner_only.arity_at_depth(0), Some(3));

    let empty = SignatureBinderScopeV1::for_declaration(0, None);
    assert_eq!(empty.available_depths(), 0);
    assert_eq!(empty.arity_at_depth(0), None);
}

#[test]
fn scope_accepts_own_and_nominal_owner_binders() {
    let scope = SignatureBinderScopeV1::for_declaration(2, Some(3));

    assert_eq!(scope.validate(&binder(0, 1)), Ok(()));
    assert_eq!(scope.validate(&binder(1, 2)), Ok(()));
}

#[test]
fn scope_rejects_out_of_range_depth_and_index() {
    let scope = SignatureBinderScopeV1::for_declaration(2, Some(3));

    assert_eq!(
        scope.validate(&binder(2, 0)),
        Err(SignatureBinderScopeError::DepthOutOfRange {
            depth: 2,
            available_depths: 2,
        })
    );
    assert_eq!(
        scope.validate(&binder(0, 2)),
        Err(SignatureBinderScopeError::IndexOutOfRange {
            depth: 0,
            index: 2,
            arity: 2,
        })
    );
}

#[test]
fn scope_traverses_every_composite_signature_shape() {
    let generic = generic_interface();
    let invalid = binder(0, 1);
    let signatures = vec![
        SignatureTypeKey::NominalApplication {
            origin: generic.id(),
            arguments: non_empty(invalid.clone()),
        },
        SignatureTypeKey::Tuple(non_empty(invalid.clone())),
        SignatureTypeKey::Function {
            effect: Effect::Ordinary,
            parameters: vec![invalid.clone()],
            result: Box::new(SignatureTypeKey::NominalApplication {
                origin: generic.id(),
                arguments: non_empty(binder(0, 0)),
            }),
        },
        SignatureTypeKey::Function {
            effect: Effect::Ordinary,
            parameters: Vec::new(),
            result: Box::new(invalid.clone()),
        },
        SignatureTypeKey::RawPointer(Box::new(invalid.clone())),
        SignatureTypeKey::NativeFunctionPointer {
            calling_convention: CallingConvention::C,
            parameters: vec![invalid.clone()],
            result: Box::new(binder(0, 0)),
        },
        SignatureTypeKey::NativeFunctionPointer {
            calling_convention: CallingConvention::C,
            parameters: Vec::new(),
            result: Box::new(invalid),
        },
    ];
    let scope = SignatureBinderScopeV1::for_declaration(1, None);

    for signature in signatures {
        assert_eq!(
            scope.validate(&signature),
            Err(SignatureBinderScopeError::IndexOutOfRange {
                depth: 0,
                index: 1,
                arity: 1,
            })
        );
    }
}

#[test]
fn binder_bounds_validate_own_and_owner_scope() {
    let generic = generic_interface();
    let own_bound = nominal_application(generic.id(), binder(0, 0));
    let owner_bound = nominal_application(generic.id(), binder(1, 0));
    let binders = CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
        CanonicalIdentifier::new("T").unwrap(),
        TypeParameterBoundsV1::Nominal(
            NominalTypeParameterBoundsV1::try_new(
                Some(own_bound),
                CanonicalSignatureTypesV1::try_new(vec![owner_bound]).unwrap(),
            )
            .unwrap(),
        ),
    )])
    .unwrap();

    assert_eq!(binders.validate_bound_scopes(Some(1)), Ok(()));
}

#[test]
fn binder_bounds_report_binder_and_bound_position() {
    let generic = generic_interface();
    let invalid = nominal_application(generic.id(), binder(1, 1));
    let binders = CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
        CanonicalIdentifier::new("T").unwrap(),
        TypeParameterBoundsV1::Nominal(
            NominalTypeParameterBoundsV1::try_new(
                None,
                CanonicalSignatureTypesV1::try_new(vec![invalid]).unwrap(),
            )
            .unwrap(),
        ),
    )])
    .unwrap();

    assert_eq!(
        binders.validate_bound_scopes(Some(1)),
        Err(TypeParameterBinderScopeValidationError {
            binder_index: 0,
            bound: TypeParameterBoundLocation::Interface { interface_index: 0 },
            error: SignatureBinderScopeError::IndexOutOfRange {
                depth: 1,
                index: 1,
                arity: 1,
            },
        })
    );
}

fn binder(depth: u32, index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth, index }
}

fn non_empty(signature: SignatureTypeKey) -> NonEmptyVec<SignatureTypeKey> {
    NonEmptyVec::new(vec![signature]).unwrap()
}

fn nominal_application(
    origin: PersistentGenericTypeId,
    argument: SignatureTypeKey,
) -> SignatureTypeKey {
    SignatureTypeKey::NominalApplication {
        origin,
        arguments: non_empty(argument),
    }
}

fn generic_interface() -> CborIdentityRecord<PersistentGenericTypeId, SourceDeclarationKey> {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site,
        CanonicalIdentifier::new("Bound").unwrap(),
        SourceNominalKind::Interface,
        1,
    ))
    .unwrap()
}
