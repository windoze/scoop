use super::*;
use scoop_hir::{IntegerKind, IntrinsicTypeKind};

#[test]
fn public_intrinsic_declarations_supply_abi_without_an_extern_witness() {
    for provider in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        let current = Fixture::empty();
        let integer =
            Fixture::intrinsic(provider, IntrinsicTypeKind::Integer(IntegerKind::SIGNED_32))
                .without_native_witness();
        let exact = integer.exact();
        let signature = ExactCallableSignature::new(Effect::Ordinary, None, vec![exact], exact);

        let actual = replay(&current, &[integer.borrow()], &signature).unwrap();
        assert!(
            matches!(actual.arguments(), [ScoopAbiArgument::Direct(value)] if value.byte_size() == 4)
        );
        assert!(matches!(actual.result(), ScoopAbiReturn::Direct(value) if value.byte_size() == 4));

        replay(&current, &[integer.borrow()], &signature).unwrap();
    }
}

#[test]
fn public_intrinsic_projection_rejects_a_foreign_provider_and_conflicting_witness() {
    let current = Fixture::empty();
    let integer = Fixture::intrinsic(
        ConeIdentity::CORE,
        IntrinsicTypeKind::Integer(IntegerKind::SIGNED_32),
    );
    let exact = integer.exact();
    let signature = ExactCallableSignature::new(Effect::Ordinary, None, vec![], exact);
    let mut foreign = integer.borrow();
    foreign.identity = ConeIdentity::SINGLE_FILE;
    assert!(matches!(
        replay(&current, &[foreign], &signature),
        Err(NativeBoundaryCompileError::NominalProvider {
            declared: ConeIdentity::CORE,
            provider: ConeIdentity::SINGLE_FILE,
            ..
        })
    ));
    let mut changed = Fixture::intrinsic(
        ConeIdentity::CORE,
        IntrinsicTypeKind::Integer(IntegerKind::SIGNED_32),
    );
    changed.nominals = Fixture::intrinsic(ConeIdentity::CORE, IntrinsicTypeKind::Boolean).nominals;
    for sources in [
        [integer.borrow(), changed.borrow()],
        [changed.borrow(), integer.borrow()],
    ] {
        assert!(matches!(
            replay(&current, &sources, &signature),
            Err(NativeBoundaryCompileError::ConflictingTypeWitness { .. })
        ));
    }
}

#[test]
fn exact_identity_alone_does_not_supply_an_intrinsic_representation() {
    let current = Fixture::empty();
    let mut missing = Fixture::intrinsic(
        ConeIdentity::CORE,
        IntrinsicTypeKind::Integer(IntegerKind::SIGNED_32),
    )
    .without_native_witness();
    missing.nominals = scoop_hir::CanonicalNominalInterfacesV1::default();
    let signature = ExactCallableSignature::new(Effect::Ordinary, None, vec![], missing.exact());
    assert!(matches!(
        replay(&current, &[missing.borrow()], &signature),
        Err(NativeBoundaryCompileError::ClosureRequired { .. })
    ));
}
